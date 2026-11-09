//! an agent may restore a file only when its newest change is its own.
mod common;
use common::Repo;

#[test]
fn an_agent_cannot_restore_over_someone_elses_newest_change() {
    let repo = Repo::new("");
    let first = repo.edit_as("agent:claude:s1", "a.txt", "one\n");
    repo.later();
    repo.edit_as("agent:codex:s2", "a.txt", "one\ntwo\n");
    let refused = repo.with("agent:claude:s1", &["restore", "a.txt", "--at", &first], None, &[]);
    assert_eq!(refused.code, 1, "{} {}", refused.out, refused.err);
    assert!(refused.err.contains("another actor changed last"), "{}", refused.err);
    assert_eq!(repo.read("a.txt"), "one\ntwo\n", "nothing was written");
    // the person may
    assert!(repo.with("human:me", &["restore", "a.txt", "--at", &first], None, &[]).ok());
    assert_eq!(repo.read("a.txt"), "one\n");
}

#[test]
fn an_agent_may_restore_its_own_newest_change() {
    let repo = Repo::new("");
    let first = repo.edit_as("agent:claude:s1", "b.txt", "one\n");
    repo.later();
    repo.edit_as("agent:claude:s1", "b.txt", "one\ntwo\n");
    let done = repo.with("agent:claude:s1", &["restore", "b.txt", "--at", &first], None, &[]);
    assert!(done.ok(), "{} {}", done.out, done.err);
    assert_eq!(repo.read("b.txt"), "one\n");
}
