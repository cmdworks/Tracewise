//! B8: `tracewise undo --session` and `--intent` revert a whole task, only its changes, and an agent only its own.
mod common;
use common::Repo;

/// a.txt, b.txt, c.txt exist from the baseline; each test then edits some of them as agents.
fn repo() -> Repo {
    let repo = Repo::uninitialised("");
    for f in ["a.txt", "b.txt", "c.txt"] {
        repo.write(f, "base\n");
    }
    assert!(repo.me(&["init"]).ok());
    repo
}

const EDITED: &str = "base\nedited\n";

#[test]
fn undo_by_session_reverts_only_that_session() {
    let repo = repo();
    repo.edit_as("agent:codex:s1", "a.txt", EDITED);
    repo.edit_as("agent:codex:s2", "b.txt", EDITED);
    repo.edit_as("agent:codex:s1", "c.txt", EDITED);

    let dry = repo.me(&["undo", "--session", "s1", "--dry-run"]);
    assert!(dry.ok(), "{}", dry.err);
    assert_eq!(repo.read("a.txt"), EDITED, "a dry run changes nothing");
    assert!(dry.out.contains("would revert") && !dry.out.contains("reverted:"), "a dry run says what it would do: {}", dry.out);

    let out = repo.me(&["undo", "--session", "s1", "--json"]);
    assert!(out.ok(), "{}", out.err);
    let mut reverted: Vec<String> = out.json()["reverted"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect();
    reverted.sort();
    assert_eq!(reverted, ["a.txt", "c.txt"]);
    assert_eq!((repo.read("a.txt"), repo.read("c.txt")), ("base\n".into(), "base\n".into()));
    assert_eq!(repo.read("b.txt"), EDITED, "the other session's file stays");
}

#[test]
fn undo_by_intent_reverts_only_the_changes_under_that_intent() {
    let repo = repo();
    let a = repo.edit_as("agent:codex:s1", "a.txt", EDITED);
    repo.edit_as("agent:codex:s1", "b.txt", EDITED);
    let note = repo.me(&["note", &a, "rename the helper across the module"]);
    assert!(note.ok(), "{}", note.err);
    let intent = repo.rows(&[]).iter().find(|c| c["id"] == a.as_str()).unwrap()["intent"].as_str().unwrap().to_string();

    let out = repo.me(&["undo", "--intent", &intent]);
    assert!(out.ok(), "{}", out.err);
    assert_eq!(repo.read("a.txt"), "base\n");
    assert_eq!(repo.read("b.txt"), EDITED, "a change of the same session under no intent stays");
}

#[test]
fn an_agent_cannot_undo_another_agents_session_but_can_undo_its_own() {
    let repo = repo();
    repo.edit_as("agent:codex:s1", "a.txt", EDITED);
    repo.run("agent:gemini:g1", &["undo", "--session", "s1"]);
    assert_eq!(repo.read("a.txt"), EDITED, "gemini may not undo codex");
    repo.run("agent:codex:s9", &["undo", "--session", "s1"]);
    assert_eq!(repo.read("a.txt"), "base\n", "codex may undo its own session from another session");
}

#[test]
fn undo_with_nothing_to_select_is_an_error() {
    let repo = repo();
    let out = repo.me(&["undo"]);
    assert!(!out.ok() && out.err.contains("--intent"), "{}", out.err);
}

#[test]
fn a_session_is_undone_however_old_it_is() {
    let repo = repo();
    // a person's session recorded long ago (a person may set the clock; an agent may not)
    repo.write("a.txt", EDITED);
    let scan = repo.with("human:me:old", &["scan"], None, &[("TRACEWISE_NOW", "2020-01-01T00:00:00Z")]);
    assert!(scan.ok(), "{}", scan.err);
    let by_actor = repo.me(&["undo", "--actor", "me", "--dry-run", "--json"]).json();
    assert!(by_actor["reverted"].as_array().unwrap().is_empty(), "--actor alone only looks back 24 hours: {by_actor}");
    let out = repo.me(&["undo", "--session", "old"]);
    assert!(out.ok(), "{}", out.err);
    assert_eq!(repo.read("a.txt"), "base\n", "a session names its changes exactly, so no time window applies");
}
