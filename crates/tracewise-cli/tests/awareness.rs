//! B7/B5: the first-prompt digest and the overlap alert. Both log to `shadow-warnings.jsonl` when `[hooks] warn` is `shadow` or
//! `on`, and are told to Claude only with `on`. They never change a decision.
mod common;
use common::Repo;

const COORD: &str = "[profile]\ncoordination = true\n";
const ON: &str = "[profile]\ncoordination = true\n[hooks]\nwarn = \"on\"\n";

fn hook(repo: &Repo, agent: &str, session: &str, event: &str, paths: &[&str]) -> serde_json::Value {
    let body = serde_json::json!({"agent": agent, "session": session, "paths": paths}).to_string();
    let run = repo.with(&format!("agent:{agent}:{session}"), &["hook", event], Some(&body), &[]);
    assert!(run.ok(), "{}", run.err);
    run.json()
}

fn notice(v: &serde_json::Value) -> String {
    v["notice"].as_str().unwrap_or("").to_string()
}

fn review_lines(repo: &Repo, kind: &str) -> usize {
    std::fs::read_to_string(repo.dir().join(".git/trail/shadow-warnings.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains(&format!("\"kind\":\"{kind}\"")))
        .count()
}

/// Claude worked here, then codex changed b.txt and a person changed c.txt.
fn others_changed_things_since_claude_left(config: &str) -> Repo {
    let repo = Repo::new(config);
    repo.edit_as("agent:claude:s0", "a.txt", "one\n");
    repo.edit_as("agent:codex:s2", "b.txt", "two\n");
    repo.edit_as("human:me", "c.txt", "three\n");
    repo
}

#[test]
fn the_first_prompt_gets_a_digest_of_what_others_changed_while_the_agent_was_away() {
    let repo = others_changed_things_since_claude_left(ON);
    let first = notice(&hook(&repo, "claude", "s1", "prompt", &[]));
    assert!(first.starts_with("Trail digest: since claude last worked here, 2 files changed by"), "{first}");
    assert!(first.contains("b.txt") && first.contains("c.txt") && first.contains("codex"), "{first}");
    assert!(!first.contains("a.txt"), "claude's own change is not reported: {first}");
    assert_eq!(notice(&hook(&repo, "claude", "s1", "prompt", &[])), "", "said once, not at the second prompt");
}

#[test]
fn no_digest_when_nothing_changed_or_the_agent_never_worked_here() {
    let repo = Repo::new(ON);
    repo.edit_as("agent:claude:s0", "a.txt", "one\n");
    assert_eq!(notice(&hook(&repo, "claude", "s1", "prompt", &[])), "", "nothing changed since claude left");
    assert_eq!(notice(&hook(&repo, "gemini", "g1", "prompt", &[])), "", "a new agent has no baseline");
}

#[test]
fn the_digest_is_logged_in_shadow_mode_but_not_told_and_not_at_all_when_off() {
    let shadow = others_changed_things_since_claude_left(COORD);
    assert_eq!(notice(&hook(&shadow, "claude", "s1", "prompt", &[])), "");
    assert_eq!(review_lines(&shadow, "digest"), 1);
    let off = others_changed_things_since_claude_left(&format!("{COORD}[hooks]\nwarn = \"off\"\n"));
    assert_eq!(notice(&hook(&off, "claude", "s1", "prompt", &[])), "");
    assert_eq!(review_lines(&off, "digest"), 0);
    let core = others_changed_things_since_claude_left("");
    assert_eq!(notice(&hook(&core, "claude", "s1", "prompt", &[])), "", "the core profile defaults to off");
}

#[test]
fn only_claude_is_told_until_other_agents_are_proven() {
    let repo = Repo::new(ON);
    repo.edit_as("agent:cursor:c0", "a.txt", "one\n");
    repo.edit_as("agent:codex:s2", "b.txt", "two\n");
    assert_eq!(notice(&hook(&repo, "cursor", "c1", "prompt", &[])), "");
    assert_eq!(review_lines(&repo, "digest"), 1, "still logged for review");
}

#[test]
fn an_overlap_is_told_to_both_sessions_once() {
    let repo = Repo::new(ON);
    repo.edit_as("agent:codex:s2", "a.txt", "one\n");
    let answer = hook(&repo, "claude", "s1", "pre-edit", &["a.txt"]);
    assert_eq!(answer["decision"], "allow", "an overlap never blocks: {answer}");
    let told = notice(&hook(&repo, "claude", "s1", "prompt", &[]));
    assert!(told.contains("Trail overlap: claude and codex are both editing a.txt"), "{told}");
    hook(&repo, "claude", "s1", "pre-edit", &["a.txt"]);
    assert_eq!(review_lines(&repo, "overlap"), 1, "once per pair of sessions and file");
    assert_eq!(notice(&hook(&repo, "claude", "s1", "prompt", &[])), "", "not repeated at the next prompt");
}

#[test]
fn the_other_session_hears_of_the_overlap_too() {
    let repo = Repo::new(ON);
    repo.edit_as("agent:claude:s2", "a.txt", "one\n");
    hook(&repo, "claude", "s1", "pre-edit", &["a.txt"]);
    let other = notice(&hook(&repo, "claude", "s2", "prompt", &[]));
    assert!(other.contains("Trail overlap:") && other.contains("a.txt"), "{other}");
}

#[test]
fn no_overlap_for_a_session_alone_or_a_file_nobody_else_touched() {
    let repo = Repo::new(ON);
    repo.edit_as("agent:claude:s1", "a.txt", "one\n");
    repo.edit_as("agent:codex:s2", "b.txt", "two\n");
    hook(&repo, "claude", "s1", "pre-edit", &["a.txt"]);
    assert_eq!(review_lines(&repo, "overlap"), 0, "the only other change is in another file; the session's own do not count");
}

#[test]
fn overlaps_are_logged_in_shadow_mode_and_told_to_nobody() {
    let repo = Repo::new(COORD);
    repo.edit_as("agent:codex:s2", "a.txt", "one\n");
    hook(&repo, "claude", "s1", "pre-edit", &["a.txt"]);
    assert_eq!(review_lines(&repo, "overlap"), 1);
    assert_eq!(notice(&hook(&repo, "claude", "s1", "prompt", &[])), "");
    assert!(!repo.dir().join(".git/trail/overlaps.jsonl").exists(), "nothing is queued for delivery");
}

#[test]
fn claude_sees_the_digest_as_it_is_not_under_the_since_your_last_prompt_header() {
    let repo = others_changed_things_since_claude_left(ON);
    let body = r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1"}"#;
    let out = repo.with("agent:claude:s1", &["hook", "UserPromptSubmit", "--from", "claude"], Some(body), &[]).out;
    assert!(out.contains("additionalContext") && out.contains("Trail digest: since claude last worked here"), "{out}");
    assert!(!out.contains("since your last prompt"), "{out}");
}

#[test]
fn a_persons_recent_edit_is_not_an_overlap_between_agents() {
    let repo = Repo::new(ON);
    // saved by a person a minute ago: older than the 20 s the coordination profile protects, newer than the 5 minute window
    let minute_ago = (chrono::Utc::now() - chrono::Duration::seconds(60)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    repo.write("a.txt", "one\n");
    assert!(repo.with("human:me", &["scan"], None, &[("TRAIL_NOW", &minute_ago)]).ok());
    let answer = hook(&repo, "claude", "s1", "pre-edit", &["a.txt"]);
    assert_eq!(answer["decision"], "allow", "{answer}");
    assert_eq!(review_lines(&repo, "overlap"), 0, "a person is not an agent session");
}

#[test]
fn a_third_session_does_not_hear_of_two_others_overlap() {
    let repo = Repo::new(ON);
    repo.edit_as("agent:claude:s2", "a.txt", "one\n");
    hook(&repo, "claude", "s1", "pre-edit", &["a.txt"]);
    assert_eq!(notice(&hook(&repo, "claude", "s3", "prompt", &[])), "");
}
