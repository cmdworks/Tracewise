//! B3: `trail file <path>` shows the lock and the latest changes of one file.
mod common;
use common::Repo;

#[test]
fn file_shows_lock_and_latest_changes_of_one_file_only() {
    let repo = Repo::new("");
    repo.edit_as("human:me", "a.txt", "one\n");
    repo.edit_as("human:me", "b.txt", "other\n");
    repo.edit_as("human:me", "a.txt", "one\ntwo\n");

    let text = repo.me(&["file", "a.txt"]).out;
    assert!(text.contains("not locked") && text.contains("modify") && text.contains("create"), "{text}");
    assert!(!text.contains("b.txt"), "{text}");

    let one = repo.me(&["file", "a.txt", "--limit", "1", "--json"]).json();
    assert_eq!(one["path"], "a.txt");
    assert!(one["lock"].is_null());
    assert_eq!(one["changes"].as_array().unwrap().len(), 1, "--limit applies");
    assert_eq!(one["changes"][0]["op"], "modify", "newest first");

    let db = tracewise_core::Database::open(&repo.dir().join(".git/trail/trail.db")).unwrap();
    let holder = tracewise_core::model::Actor { kind: "agent".into(), name: Some("claude".into()), session: Some("s1".into()) };
    assert!(db.acquire_lock("a.txt", &holder, None, 600).unwrap().is_none(), "None means taken");
    let text = repo.me(&["file", "a.txt"]).out;
    assert!(text.contains("locked by"), "{text}");
}

#[test]
fn file_without_history_says_so() {
    let repo = Repo::new("");
    assert!(repo.me(&["file", "nothing.txt"]).out.contains("no recorded changes"));
}

#[test]
fn file_accepts_dot_and_absolute_paths() {
    let repo = Repo::new("");
    repo.edit_as("human:me", "a.txt", "one\n");
    let abs = std::fs::canonicalize(repo.dir().join("a.txt")).unwrap();
    for arg in ["./a.txt", abs.to_str().unwrap()] {
        let text = repo.me(&["file", arg]).out;
        assert!(text.contains("create"), "{arg}: {text}");
    }
}

#[test]
fn file_rejects_a_path_outside_the_repository() {
    let repo = Repo::new("");
    let out = repo.me(&["file", "../outside.txt"]);
    assert!(out.out.contains("no recorded changes") || !out.ok(), "{}{}", out.out, out.err);
}
