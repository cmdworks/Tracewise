//! rebuild, fetch and push must not report success after losing data.
mod common;
use common::{Repo, Run};
use std::path::PathBuf;
use std::process::Command;

/// A repository sharing through a bare remote that lives outside it.
struct Shared {
    repo: Repo,
    _remote_dir: tempfile::TempDir,
    remote: PathBuf,
}

impl Shared {
    fn trail(&self, args: &[&str]) -> Run {
        self.repo.with("human:me", args, None, &[("TRAIL_PROFILE", "full")])
    }
    fn dir(&self) -> &std::path::Path {
        self.repo.dir()
    }
    fn head(&self) -> String {
        std::fs::read_to_string(self.dir().join(".git/trail/chain-head")).unwrap_or_default()
    }
    fn change_count(&self) -> usize {
        self.trail(&["log", "--json"]).json().as_array().unwrap().len()
    }
}

/// A repository with three changes, shared and pushed once.
fn shared_repo() -> Shared {
    let remote_dir = tempfile::tempdir().unwrap();
    let remote = remote_dir.path().join("meta.git");
    let ok = Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success();
    assert!(ok);
    let repo = Repo::uninitialised("");
    repo.write("a.txt", "one\n");
    let shared = Shared { repo, _remote_dir: remote_dir, remote };
    assert!(shared.trail(&["init"]).ok());
    let url = format!("file://{}", shared.remote.display());
    assert!(shared.trail(&["share", "init", "--remote", &url, "--level", "full"]).ok());
    shared.repo.write("a.txt", "one\ntwo\n");
    shared.repo.write("b.txt", "bee\n");
    assert!(shared.trail(&["scan"]).ok());
    let pushed = shared.trail(&["push"]);
    assert!(pushed.ok(), "{}", pushed.err);
    shared
}

#[test]
fn rebuild_keeps_the_checkpoint_and_the_history() {
    let repo = shared_repo();
    let (count, head) = (repo.change_count(), repo.head());
    assert!(!head.is_empty());
    let out = repo.trail(&["rebuild"]);
    assert!(out.ok(), "{}", out.err);
    assert_eq!(repo.change_count(), count);
    assert!(!repo.head().is_empty(), "the chain-head checkpoint must survive a rebuild");
    assert!(repo.trail(&["verify"]).ok());
}

#[test]
fn a_rebuild_that_fails_changes_nothing() {
    let repo = shared_repo();
    let (count, head) = (repo.change_count(), repo.head());
    let batches = repo.dir().join(".git/trail/batches");
    let batch = std::fs::read_dir(&batches).unwrap().flatten().find(|e| e.path().is_dir()).unwrap().path();
    std::fs::write(batch.join("changes.jsonl.zst"), b"not zstd").unwrap();
    let out = repo.trail(&["rebuild"]);
    assert!(!out.ok(), "a damaged batch must fail the rebuild");
    assert_eq!(repo.change_count(), count, "history untouched");
    assert_eq!(repo.head(), head, "checkpoint untouched");

    std::fs::remove_dir_all(&batches).unwrap();
    std::fs::remove_dir_all(repo.dir().join(".git/trail/remote")).unwrap();
    let out = repo.trail(&["rebuild"]);
    assert!(!out.ok(), "no batches: nothing to rebuild from");
    assert_eq!(repo.change_count(), count);
}

#[test]
fn rebuild_never_removes_what_is_in_no_batch() {
    let repo = shared_repo();
    repo.repo.write("a.txt", "one\ntwo\nlocal only\n");
    assert!(repo.trail(&["scan"]).ok());
    let (count, head) = (repo.change_count(), repo.head());
    let out = repo.trail(&["rebuild"]);
    assert!(out.ok(), "{}", out.err);
    assert_eq!(repo.change_count(), count, "the unpushed change and the baseline stay");
    assert_eq!(repo.head(), head);
    assert!(repo.trail(&["verify"]).ok());
}

#[test]
fn rebuild_into_a_new_database_restores_the_pushed_history() {
    let repo = shared_repo();
    let pushed: Vec<String> = {
        repo.trail(&["log", "--json"])
            .json()
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["op"] != "baseline")
            .map(|c| c["id"].as_str().unwrap().to_string())
            .collect()
    };
    let data = repo.dir().join(".git/trail");
    for f in ["trail.db", "trail.db-wal", "trail.db-shm", "chain-head"] {
        let _ = std::fs::remove_file(data.join(f));
    }
    let out = repo.trail(&["rebuild"]);
    assert!(out.ok(), "{}", out.err);
    let rows = repo.trail(&["log", "--json"]).json();
    let got: Vec<&str> = rows.as_array().unwrap().iter().filter(|c| c["op"] != "baseline").map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(got.len(), pushed.len());
    assert!(pushed.iter().all(|id| got.contains(&id.as_str())));
    assert!(repo.trail(&["verify"]).ok());
}

#[test]
fn fetch_from_an_unreachable_remote_is_an_error() {
    let repo = shared_repo();
    std::fs::remove_dir_all(&repo.remote).unwrap();
    let out = repo.trail(&["fetch"]);
    assert!(!out.ok(), "{}", out.out);
}

#[test]
fn push_refuses_to_publish_a_change_whose_version_is_gone() {
    let repo = shared_repo();
    repo.repo.write("a.txt", "one\ntwo\nthree\n");
    assert!(repo.trail(&["scan"]).ok());
    let rows = repo.trail(&["log", "a.txt", "--json"]).json();
    let newest = rows.as_array().unwrap()[0]["after"].as_str().unwrap().to_string();
    let loose = repo.dir().join(".git/trail/objects").join(&newest[..2]).join(&newest[2..]);
    std::fs::remove_file(loose).unwrap();
    let out = repo.trail(&["push"]);
    assert!(!out.ok(), "{}", out.out);
    assert!(out.err.contains("unreadable"));
    let heads = Command::new("git").args(["ls-remote", repo.remote.to_str().unwrap()]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&heads.stdout).matches("trail/device").count(), 1, "still only the first batch");
}

/// sharing and local storage use one secret scanner; a file with a key is pushed as a change without any content.
#[test]
fn a_file_with_a_secret_is_pushed_without_its_content() {
    let repo = shared_repo();
    let key = format!("ghp_{}", "A1b2C3d4E5".repeat(4));
    repo.repo.write("cfg.txt", &format!("token = {key}\n"));
    assert!(repo.trail(&["scan"]).ok());
    let pushed = repo.trail(&["push"]);
    assert!(pushed.ok(), "{}", pushed.err);
    assert!(repo.trail(&["verify"]).ok());
    // nothing under the shared data holds the key, packed or loose
    let mut stack = vec![repo.dir().join(".git/trail"), repo.remote.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(b) = std::fs::read(&p) {
                assert!(!String::from_utf8_lossy(&b).contains(&key), "key found in {p:?}");
            }
        }
    }
}
