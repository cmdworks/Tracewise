//! B4/section 11: in the `coordination` profile the pre-edit hook notes a file that someone else changed after the session's last
//! prompt in `shadow-warnings.jsonl`. `[hooks] warn = "on"` also tells Claude, once per change; the decision stays `allow`.
mod common;
use common::Repo;

const COORD: &str = "[profile]\ncoordination = true\n";
const PRE: &str = r#"{"agent":"claude","session":"s1","paths":["a.txt"]}"#;
const PROMPT: &str = r#"{"agent":"claude","session":"s1"}"#;
const CLAUDE_PRE: &str = r#"{"hook_event_name":"PreToolUse","session_id":"s1","tool_name":"Edit","tool_input":{"file_path":"a.txt"}}"#;

/// Session s1 has prompted; afterwards codex changes a.txt. (The first version is written by an agent too: in the coordination
/// profile a file a person saved seconds ago is denied to agents, which is not what these tests are about.)
fn other_changes_a_file(config: &str) -> Repo {
    let repo = Repo::new(config);
    repo.edit_as("agent:codex:s0", "a.txt", "one\n");
    assert!(repo.with("agent:claude:s1", &["hook", "prompt"], Some(PROMPT), &[]).ok());
    repo.edit_as("agent:codex:s2", "a.txt", "one\ntwo\n");
    repo
}

/// The review lines of this warning (the file also holds the digest and overlap lines of `awareness.rs`).
fn shadow(repo: &Repo) -> String {
    let all = std::fs::read_to_string(repo.dir().join(".git/trail/shadow-warnings.jsonl")).unwrap_or_default();
    all.lines().filter(|l| l.contains("\"kind\":\"edit\"")).collect::<Vec<_>>().join("\n")
}

fn pre_edit(repo: &Repo) -> String {
    repo.with("agent:claude:s1", &["hook", "pre-edit"], Some(PRE), &[]).out
}

fn claude_pre(repo: &Repo, from: &str) -> String {
    repo.with("agent:claude:s1", &["hook", "PreToolUse", "--from", from], Some(CLAUDE_PRE), &[]).out
}

#[test]
fn a_change_by_someone_else_is_noted_but_the_agent_hears_nothing() {
    let repo = Repo::new(COORD);
    repo.edit_as("agent:codex:s0", "a.txt", "one\n");
    repo.with("agent:claude:s1", &["hook", "prompt"], Some(PROMPT), &[]);
    repo.later();
    let quiet = pre_edit(&repo);
    assert!(quiet.contains("\"allow\""), "{quiet}");
    assert_eq!(shadow(&repo), "", "nobody else changed it yet");

    repo.edit_as("agent:codex:s2", "a.txt", "one\ntwo\n");
    let out = pre_edit(&repo);
    assert_eq!(out, quiet, "the agent's answer is the same as before the other change");
    let line = shadow(&repo);
    assert!(line.contains("\"path\":\"a.txt\"") && line.contains("\"session\":\"s1\"") && line.contains("codex"), "{line}");
}

#[test]
fn the_core_profile_writes_nothing() {
    let repo = other_changes_a_file("");
    pre_edit(&repo);
    assert_eq!(shadow(&repo), "");
}

#[test]
fn with_warn_on_claude_is_told_once_and_the_edit_is_still_allowed() {
    let repo = other_changes_a_file(&format!("{COORD}[hooks]\nwarn = \"on\"\n"));
    let first = claude_pre(&repo, "claude");
    assert!(first.contains("additionalContext") && first.contains("a.txt was changed by codex (exact)"), "{first}");
    assert!(first.contains("Re-read before editing") && !first.contains("deny"), "{first}");
    assert_eq!(claude_pre(&repo, "claude").trim(), "{}", "the same change is not repeated");
}

#[test]
fn with_warn_shadow_or_off_the_agent_hears_nothing() {
    for (config, logged) in [(COORD.to_string(), true), (format!("{COORD}[hooks]\nwarn = \"off\"\n"), false)] {
        let repo = other_changes_a_file(&config);
        assert_eq!(claude_pre(&repo, "claude").trim(), "{}", "{config}");
        assert_eq!(!shadow(&repo).is_empty(), logged, "{config}");
    }
}

#[test]
fn the_environment_variable_overrides_the_config() {
    let repo = other_changes_a_file(&format!("{COORD}[hooks]\nwarn = \"off\"\n"));
    let out = repo.with("agent:claude:s1", &["hook", "PreToolUse", "--from", "claude"], Some(CLAUDE_PRE), &[("TRAIL_WARN", "on")]).out;
    assert!(out.contains("a.txt was changed by"), "{out}");
}

#[test]
fn only_claude_gets_the_warning_until_other_agents_are_proven() {
    let repo = other_changes_a_file(&format!("{COORD}[hooks]\nwarn = \"on\"\n"));
    assert_eq!(claude_pre(&repo, "cursor").trim(), "{}");
}
