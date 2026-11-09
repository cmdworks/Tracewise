//! an agent cannot switch a shim off by setting `TRAIL_SHIM_DEPTH` itself.
//! a shimmed command that fails half way still has its real changes recorded and keeps its own exit status.
mod common;
use common::Repo;

#[test]
fn a_caller_set_loop_guard_does_not_skip_recording() {
    let repo = Repo::uninitialised("");
    repo.write("f.txt", "alpha\n");
    assert!(repo.me(&["init"]).ok());

    // `trail` started as `sed`, with the guard set by its caller (this test, not Tracewise)
    let out = repo.as_program("sed", &["-i", "", "s/alpha/beta/", "f.txt"], &[("TRAIL_SHIM_DEPTH", "1")]);
    assert!(out.ok(), "{}", out.err);
    assert_eq!(repo.read("f.txt"), "beta\n");

    let rows = repo.rows(&["f.txt"]);
    assert!(rows.iter().any(|c| c["source"] == "shim" && c["op"] == "modify"), "the edit is recorded by the shim: {rows:?}");
}

#[test]
fn a_shim_command_that_fails_midway_records_what_it_really_changed() {
    let repo = Repo::uninitialised("");
    repo.write("a.txt", "a\n");
    repo.write("b.txt", "b\n");
    assert!(repo.me(&["init"]).ok());

    let out = repo.as_program("rm", &["a.txt", "missing.txt", "b.txt"], &[]);
    assert_eq!(out.code, 1, "rm's own failure is kept");
    assert!(out.err.contains("missing.txt"));
    let mut deleted: Vec<String> =
        repo.rows(&[]).iter().filter(|c| c["op"] == "delete").map(|c| c["path"].as_str().unwrap().to_string()).collect();
    deleted.sort();
    assert_eq!(deleted, ["a.txt", "b.txt"]);
}
