//! B9/B10: `tracewise log --actor/--since/--grep` really filter (they were once accepted and ignored), `--group` clusters by task and
//! `tracewise report` writes one task as Markdown.
mod common;
use common::Repo;

/// a.txt and c.txt are edited by codex session s1, then b.txt by gemini g1, which is the newest change of all.
fn repo() -> Repo {
    let repo = Repo::new("");
    repo.edit_as("agent:codex:s1", "a.txt", "base\n");
    repo.edit_as("agent:codex:s1", "c.txt", "base\n");
    repo.edit_as("agent:gemini:g1", "b.txt", "base\n");
    repo
}

fn paths(rows: &[serde_json::Value]) -> Vec<String> {
    rows.iter().map(|r| r["path"].as_str().unwrap().to_string()).collect()
}

#[test]
fn actor_filters_and_the_limit_applies_after_it() {
    let repo = repo();
    let codex = repo.rows(&["--actor", "codex"]);
    assert!(codex.iter().all(|r| r["actor"]["name"] == "codex"), "{codex:?}");
    assert_eq!(paths(&codex), ["c.txt", "a.txt"], "newest first, gemini's b.txt left out");
    let one = repo.rows(&["--actor", "codex", "--limit", "1"]);
    assert_eq!(paths(&one), ["c.txt"], "the limit is taken from the filtered rows, not before filtering");
}

#[test]
fn since_keeps_only_newer_changes() {
    let repo = repo();
    let newest = repo.rows(&[])[0]["time"]["start"].as_str().unwrap().to_string();
    let cutoff = chrono::DateTime::parse_from_rfc3339(&newest).unwrap() + chrono::Duration::milliseconds(1);
    repo.edit_as("agent:codex:s1", "d.txt", "new\n");
    let recent = repo.rows(&["--since", &cutoff.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)]);
    assert_eq!(paths(&recent), ["d.txt"]);
}

#[test]
fn grep_matches_path_and_reason_case_insensitively() {
    let repo = repo();
    assert_eq!(paths(&repo.rows(&["--grep", "B.TXT"])), ["b.txt"]);
    assert!(repo.rows(&["--grep", "no-such-text"]).is_empty());
    let id = repo.rows(&["--actor", "gemini"])[0]["id"].as_str().unwrap().to_string();
    assert!(repo.me(&["note", &id, "tighten the retry budget"]).ok());
    let hit = repo.rows(&["--grep", "RETRY budget"]);
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0]["id"], id.as_str());
}

#[test]
fn group_puts_the_changes_of_a_task_together() {
    let repo = repo();
    let keys: Vec<String> = repo.rows(&["--group"]).iter().map(|r| r["group"].as_str().unwrap().to_string()).collect();
    let s1: Vec<_> = keys.iter().enumerate().filter(|(_, k)| *k == "session:s1").map(|(i, _)| i).collect();
    assert!(s1.len() >= 2 && keys.contains(&"session:g1".to_string()), "{keys:?}");
    assert_eq!(s1[s1.len() - 1] - s1[0] + 1, s1.len(), "the changes of session s1 are next to each other: {keys:?}");
    let text = repo.me(&["log", "--group"]).out;
    assert!(text.contains("== session:s1 (") && text.contains("== session:g1 (1 change)"), "{text}");
}

#[test]
fn report_lists_one_task_as_markdown() {
    let repo = repo();
    let id = repo.rows(&["--actor", "codex"]).iter().find(|r| r["path"] == "a.txt").unwrap()["id"].as_str().unwrap().to_string();
    assert!(repo.me(&["note", &id, "tighten the retry budget"]).ok());
    let out = repo.me(&["report", "--session", "s1"]);
    assert!(out.ok(), "{}", out.err);
    let md = out.out;
    assert!(md.starts_with("# Tracewise report") && md.contains("## Files") && md.contains("## Changes"), "{md}");
    assert!(md.contains("`a.txt`") && md.contains("`c.txt`") && !md.contains("`b.txt`"), "only session s1: {md}");
    assert!(md.contains("tighten the retry budget") && md.contains("no reason recorded"), "{md}");

    let json = repo.me(&["report", "--session", "s1", "--json"]).json();
    assert_eq!(json["changes"].as_array().unwrap().len(), 2);
    assert!(!repo.me(&["report", "--session", "nope"]).ok(), "an empty selection is an error, not an empty report");
}

#[test]
fn after_returns_only_what_is_newer_than_a_row_the_client_has() {
    let repo = repo();
    let rows = repo.rows(&[]);
    let last_seen = rows[1]["id"].as_str().unwrap().to_string(); // the second newest
    assert_eq!(paths(&repo.rows(&["--after", &last_seen])), [rows[0]["path"].as_str().unwrap()]);
    let newest = rows[0]["id"].as_str().unwrap().to_string();
    assert!(repo.rows(&["--after", &newest]).is_empty(), "nothing is newer than the newest");
}
