//! Git wrapper unit tests.
use super::*;

#[test]
fn test_find_real_git_skips_tracewise_binaries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir1 = tmp.path().join("dir1");
    let dir2 = tmp.path().join("dir2");
    std::fs::create_dir_all(&dir1).unwrap();
    std::fs::create_dir_all(&dir2).unwrap();

    let link1 = dir1.join("git");
    let link2 = dir2.join("git");
    let fake_tracewise = tmp.path().join("tracewise");
    std::fs::write(&fake_tracewise, b"binary").unwrap();
    tracewise_platform::shims::create_shim_link(&fake_tracewise, &link1).unwrap();
    tracewise_platform::shims::create_shim_link(&fake_tracewise, &link2).unwrap();

    let old_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{}:{}", dir1.display(), dir2.display(), old_path);
    let real = find_real_git_inner(Some(&test_path), None);
    assert_ne!(real, link1);
    assert_ne!(real, link2);
}

#[test]
fn test_git_wrapper_multiple_wrappers_no_loop() {
    let _lock = crate::TEST_MUTEX.lock().unwrap();
    let status = std::process::Command::new("cargo").args(["build", "--bin", "tracewise"]).status().expect("cargo build failed");
    assert!(status.success(), "cargo build must succeed");

    let test_exe = std::env::current_exe().expect("current exe");
    let debug_dir = test_exe.parent().and_then(|p| p.parent()).expect("target dir");
    let tracewise_bin = debug_dir.join("tracewise");
    assert!(tracewise_bin.exists(), "tracewise binary exists at {:?}", tracewise_bin);

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir1 = tmp.path().join("dir1");
    let dir2 = tmp.path().join("dir2");
    let bin_dir1 = tmp.path().join("bin1");
    let bin_dir2 = tmp.path().join("bin2");
    std::fs::create_dir_all(&dir1).unwrap();
    std::fs::create_dir_all(&dir2).unwrap();
    std::fs::create_dir_all(&bin_dir1).unwrap();
    std::fs::create_dir_all(&bin_dir2).unwrap();

    let copy1 = bin_dir1.join("tracewise");
    let copy2 = bin_dir2.join("tracewise");
    std::fs::copy(&tracewise_bin, &copy1).expect("copy 1");
    std::fs::copy(&tracewise_bin, &copy2).expect("copy 2");

    let _ = tracewise_platform::shims::set_executable(&copy1);
    let _ = tracewise_platform::shims::set_executable(&copy2);

    let link1 = dir1.join("git");
    let link2 = dir2.join("git");
    tracewise_platform::shims::create_shim_link(&copy1, &link1).expect("symlink 1");
    tracewise_platform::shims::create_shim_link(&copy2, &link2).expect("symlink 2");

    let current_path = std::env::var("PATH").unwrap_or_default();
    let custom_path = format!("{}:{}:{}", dir1.display(), dir2.display(), current_path);

    let mut child = std::process::Command::new(&link1)
        .arg("version")
        .env("PATH", &custom_path)
        .env_remove("TRACEWISE_GIT_DEPTH")
        .env_remove("TRACEWISE_REAL_GIT")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn wrapper");

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);
    let mut finished = false;
    while start.elapsed() < timeout {
        if let Ok(Some(_)) = child.try_wait() {
            finished = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if !finished {
        let _ = child.kill();
    }
    let status = child.wait().expect("reap child");
    if !finished {
        panic!("git command timed out after 5 seconds (infinite loop detected)");
    }
    assert!(status.success(), "git command succeeded");

    let mut output_str = String::new();
    if let Some(mut out) = child.stdout.take() {
        use std::io::Read;
        let _ = out.read_to_string(&mut output_str);
    }
    assert!(output_str.contains("git version"), "output should contain 'git version', got: {}", output_str);
}

#[test]
fn test_git_subcommand_classification_s01_to_s04() {
    let read_only_calls = [
        vec!["status"],
        vec!["status", "--porcelain"],
        vec!["log"],
        vec!["log", "-1"],
        vec!["diff"],
        vec!["show", "HEAD"],
        vec!["for-each-ref"],
        vec!["stash", "list"],
        vec!["stash", "show"],
        vec!["worktree", "list"],
        vec!["rev-parse", "HEAD"],
        vec!["branch", "-l"],
        vec!["--version"],
        vec!["--help"],
    ];
    for args in &read_only_calls {
        assert!(is_read_only_git(args), "expected read-only: {:?}", args);
        assert!(!is_worktree_mutating_git(args), "expected not mutating: {:?}", args);
    }

    let s02_calls = [vec!["checkout", "main"], vec!["switch", "main"], vec!["reset", "--hard", "HEAD"], vec!["restore", "file.txt"]];
    for args in &s02_calls {
        assert!(is_worktree_mutating_git(args), "expected mutating: {:?}", args);
    }

    let s03_calls = [
        vec!["commit", "-m", "test"],
        vec!["merge", "feature"],
        vec!["rebase", "main"],
        vec!["pull"],
        vec!["cherry-pick", "HEAD^"],
        vec!["revert", "HEAD"],
        vec!["am", "patch"],
    ];
    for args in &s03_calls {
        assert!(is_worktree_mutating_git(args), "expected mutating: {:?}", args);
    }

    let s04_calls = [
        vec!["stash"],
        vec!["stash", "push", "-m", "work"],
        vec!["stash", "pop"],
        vec!["stash", "apply"],
        vec!["clean", "-fd"],
        vec!["apply", "file.diff"],
    ];
    for args in &s04_calls {
        assert!(is_worktree_mutating_git(args), "expected mutating: {:?}", args);
    }
}

#[test]
fn test_read_only_git_fast_path_does_not_open_db_s05() {
    let _lock = crate::TEST_MUTEX.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let status = std::process::Command::new("git").arg("init").current_dir(&repo_dir).status().unwrap();
    assert!(status.success());

    let data_dir = repo_dir.join(".git/tracewise");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = data_dir.join("tracewise.db");
    {
        let db = Database::open(&db_path).unwrap();
        let cmds = db.list_commands().unwrap();
        assert!(cmds.is_empty());
    }

    let _ = tracewise_platform::shims::set_mode(&db_path, 0o000);

    let real_git = find_real_git(None);
    let out = std::process::Command::new(&real_git).arg("-C").arg(&repo_dir).arg("status").output().unwrap();
    assert!(out.status.success(), "git status must exit 0 even with unreadable DB");

    let _ = tracewise_platform::shims::set_mode(&db_path, 0o644);
}
