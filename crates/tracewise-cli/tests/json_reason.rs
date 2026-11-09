//! `log --json` and `show --json` carry the reason text, not only the intent id.
mod common;
use common::Repo;

#[test]
fn json_rows_have_the_reason_text() {
    let repo = Repo::new("");
    let id = repo.edit_as("human:me", "a.txt", "one\n");
    assert!(repo.me(&["note", &id, "rename the helper"]).ok());
    let other = repo.edit_as("human:me", "b.txt", "two\n");

    let rows = repo.rows(&[]);
    let row = rows.iter().find(|c| c["id"] == id.as_str()).unwrap();
    assert_eq!(row["why"], "rename the helper");
    assert_ne!(row["intent"], row["why"], "the intent is an id");
    assert_eq!(repo.me(&["show", &id, "--json"]).json()["why"], "rename the helper");
    assert!(rows.iter().find(|c| c["id"] == other.as_str()).unwrap()["why"].is_null(), "a change without a note has no reason");
}

#[test]
fn show_prints_the_reason_only_when_there_is_one() {
    let repo = Repo::new("");
    let id = repo.edit_as("human:me", "a.txt", "one\n");
    assert!(repo.me(&["note", &id, "rename the helper"]).ok());
    let other = repo.edit_as("human:me", "b.txt", "two\n");

    let text = repo.me(&["show", &id]).out;
    assert!(text.lines().any(|l| l == "Why:    rename the helper"), "{text}");
    let bare = repo.me(&["show", &other]).out;
    assert!(!bare.contains("Why:"), "{bare}");
}
