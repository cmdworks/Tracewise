//! `[share] refs = "hidden"` keeps the batches under `refs/tracewise/device/` (no branches on the remote);
//! the default stays `refs/heads/tracewise/device/`, and fetch reads both.
mod common;
use common::Repo;
use std::process::Command;

fn tracewise(repo: &Repo, args: &[&str]) -> common::Run {
    repo.with("human:me", args, None, &[("TRACEWISE_PROFILE", "full")])
}

fn remote_refs(remote: &std::path::Path) -> String {
    let out =
        Command::new("git").arg(format!("--git-dir={}", remote.display())).args(["for-each-ref", "--format=%(refname)"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn sharing_repo(url: &str, hidden: bool) -> Repo {
    let repo = Repo::uninitialised("");
    repo.write("a.txt", "one\n");
    assert!(tracewise(&repo, &["init"]).ok());
    assert!(tracewise(&repo, &["share", "init", "--remote", url, "--level", "full"]).ok());
    if hidden {
        let cfg = std::fs::read_to_string(repo.dir().join(".tracewiseconfig")).unwrap();
        repo.write(".tracewiseconfig", &format!("{cfg}refs = \"hidden\"\n"));
    }
    repo.write("a.txt", "one\ntwo\n");
    assert!(tracewise(&repo, &["scan"]).ok());
    let pushed = tracewise(&repo, &["push"]);
    assert!(pushed.ok(), "{}", pushed.err);
    repo
}

#[test]
fn hidden_refs_leave_no_branch_and_another_device_can_fetch_them() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("meta.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());

    let first = sharing_repo(&url, true);
    let refs = remote_refs(&remote);
    assert!(refs.contains("refs/tracewise/device/"), "{refs}");
    assert!(!refs.contains("refs/heads/"), "no branch may appear on the remote: {refs}");

    let second = Repo::uninitialised("");
    second.write("a.txt", "one\n");
    assert!(tracewise(&second, &["init"]).ok());
    assert!(tracewise(&second, &["share", "init", "--remote", &url, "--level", "full"]).ok());
    let fetched = tracewise(&second, &["fetch"]);
    assert!(fetched.ok(), "{}", fetched.err);
    let theirs = tracewise(&second, &["log", "--json"]).json().as_array().unwrap().len();
    assert!(
        theirs >= tracewise(&first, &["log", "--json"]).json().as_array().unwrap().len(),
        "the second device got the first one's records"
    );
}

#[test]
fn without_the_setting_the_batches_stay_on_a_branch() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("meta.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let _repo = sharing_repo(&format!("file://{}", remote.display()), false);
    let refs = remote_refs(&remote);
    assert!(refs.contains("refs/heads/tracewise/device/") && !refs.contains("refs/tracewise/"), "{refs}");
}

#[test]
fn the_companion_copy_does_not_take_the_projects_own_branches() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("project.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    // the "project" remote already holds a branch with history, as a real project's origin does
    let seed = d.path().join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git").current_dir(&seed).args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).output().unwrap();
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(seed.join("code.txt"), "project code\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "project"]);
    git(&["push", "-q", remote.to_str().unwrap(), "main"]);

    let repo = sharing_repo(&format!("file://{}", remote.display()), true);
    let companion = repo.dir().join(".git/tracewise/remote");
    let mine = remote_refs(&companion);
    assert!(
        mine.contains("refs/tracewise/device/") && !mine.contains("refs/heads/main"),
        "the companion holds only Tracewise's refs: {mine}"
    );
    // a second push still has a parent, though the companion never cloned the remote
    repo.write("a.txt", "one\ntwo\nthree\n");
    assert!(tracewise(&repo, &["scan"]).ok());
    let again = tracewise(&repo, &["push"]);
    assert!(again.ok(), "{}", again.err);
    let on_remote = remote_refs(&remote);
    assert!(on_remote.contains("refs/heads/main") && on_remote.contains("refs/tracewise/device/"), "{on_remote}");
}

#[test]
fn share_init_without_a_remote_uses_origin_hidden_refs_and_summary_and_keeps_other_sections() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("project.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());
    let repo = Repo::uninitialised("[profile]\nshare = true\n");
    repo.write("a.txt", "one\n");
    assert!(tracewise(&repo, &["init"]).ok());

    let none = tracewise(&repo, &["share", "init", "--json"]);
    assert!(!none.ok() && none.err.contains("no `origin` remote"), "{}", none.err);

    assert!(Command::new("git").current_dir(repo.dir()).args(["remote", "add", "origin", &url]).status().unwrap().success());
    let init = tracewise(&repo, &["share", "init", "--json"]);
    assert!(init.ok(), "{}", init.err);
    let v = init.json();
    assert_eq!((v["store"].as_str(), v["level"].as_str(), v["refs"].as_str()), (Some("origin"), Some("summary"), Some("hidden")), "{v}");
    let cfg = repo.read(".tracewiseconfig");
    assert!(cfg.contains("share = true"), "the profile section is kept: {cfg}");
    assert!(cfg.contains("level = \"summary\"") && cfg.contains("refs = \"hidden\""), "{cfg}");

    repo.write("a.txt", "one\ntwo\n");
    assert!(tracewise(&repo, &["scan"]).ok());
    assert!(tracewise(&repo, &["push"]).ok());
    let refs = remote_refs(&remote);
    assert!(refs.contains("refs/tracewise/device/") && !refs.contains("refs/heads/tracewise/"), "{refs}");

    // an explicit URL keeps the branch form unless --hidden is given
    let explicit = tracewise(&repo, &["share", "init", "--remote", &url, "--level", "full", "--json"]);
    assert_eq!(explicit.json()["refs"], "branches");
    assert_eq!(tracewise(&repo, &["share", "init", "--remote", &url, "--hidden", "--json"]).json()["refs"], "hidden");
}
