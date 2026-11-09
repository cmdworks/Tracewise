//! `tracewise prune` removes the history of ignored paths, keeps `verify` green, refuses agents and asks before it acts.
mod common;
use common::Repo;

/// build/ files are recorded first, then ignored, like the 126,000 `target/` records that started this.
fn repo() -> Repo {
    let repo = Repo::uninitialised("");
    repo.write("a.txt", "one\n");
    assert!(repo.me(&["init"]).ok());
    for i in 0..4 {
        repo.write(&format!("build/o{i}.txt"), "obj\n");
    }
    repo.later();
    assert!(repo.me(&["scan"]).ok());
    repo.write("a.txt", "two\n");
    repo.later();
    assert!(repo.me(&["scan"]).ok());
    repo.write(".gitignore", "build/\n");
    repo.later();
    assert!(repo.me(&["scan"]).ok()); // records the 4 builds as deleted from now on
    repo.write("a.txt", "three\n"); // the newest record is never pruned, so make it a normal one
    repo.later();
    assert!(repo.me(&["scan"]).ok());
    repo
}

fn count(repo: &Repo) -> usize {
    repo.rows(&[]).len()
}

#[test]
fn prune_removes_ignored_history_and_verify_still_passes() {
    let repo = repo();
    let before = count(&repo);
    let dry = repo.me(&["prune", "--dry-run", "--json"]);
    assert!(dry.ok(), "{}", dry.err);
    assert_eq!(dry.json()["removed"], 8, "4 creates and 4 deletes");
    assert_eq!(count(&repo), before, "a dry run removes nothing");

    let out = repo.me(&["prune", "--yes", "--json"]);
    assert!(out.ok(), "{}", out.err);
    assert_eq!(out.json()["removed"], 8);
    assert_eq!(count(&repo), before - 8);
    let verify = repo.me(&["verify"]);
    assert!(verify.ok(), "{}{}", verify.out, verify.err);
    assert_eq!(repo.me(&["prune", "--dry-run", "--json"]).json()["removed"], 0, "nothing left to prune");
    assert!(repo.me(&["scan"]).ok());
    assert_eq!(count(&repo), before - 8, "pruned paths are not recorded again");
}

#[test]
fn prune_needs_yes_without_a_terminal_and_refuses_agents() {
    let repo = repo();
    let before = count(&repo);
    let no_yes = repo.me(&["prune"]);
    assert!(!no_yes.ok());
    assert!(no_yes.err.contains("--yes"), "{}", no_yes.err);
    let agent = repo.run("agent:codex:s1", &["prune", "--yes"]);
    assert!(!agent.ok());
    assert!(agent.err.contains("agents cannot prune history"), "{}", agent.err);
    let look = repo.run("agent:codex:s1", &["prune", "--dry-run", "--json"]);
    assert!(look.ok() && look.json()["removed"] == 8, "an agent may look with --dry-run: {}", look.err);
    assert_eq!(count(&repo), before);
}

#[test]
fn prune_path_and_older_than_pick_the_records_and_backup_is_written() {
    let repo = repo();
    let none = repo.me(&["prune", "--older-than", "1d", "--dry-run", "--json"]);
    assert_eq!(none.json()["removed"], 0, "everything is newer than a day");
    let bad = repo.me(&["prune", "--older-than", "soon", "--dry-run"]);
    assert!(!bad.ok());

    let backup = repo.home().join("backup.db");
    let out =
        repo.me(&["prune", "--path", "build/o0.txt", "--path", "build/o1.txt", "--yes", "--backup", backup.to_str().unwrap(), "--json"]);
    assert!(out.ok(), "{}", out.err);
    assert_eq!(out.json()["removed"], 4, "create and delete of two files");
    assert!(backup.is_file());
    assert!(repo.me(&["verify"]).ok());
}

#[test]
fn doctor_alerts_about_prune_candidates_until_they_are_pruned() {
    let repo = repo();
    let doctor = repo.me(&["doctor"]);
    assert!(doctor.out.contains("prune candidates: 8"), "{}", doctor.out);
    assert_eq!(repo.me(&["doctor", "--json"]).json()["prune_candidates"], 8);
    assert!(repo.me(&["prune", "--yes"]).ok());
    assert!(!repo.me(&["doctor"]).out.contains("prune candidates"));
}
