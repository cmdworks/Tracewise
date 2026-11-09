pub use super::TEST_MUTEX as ENV_MUTEX;
use super::*;
use tracewise_core::model::{Actor, Intent};

#[test]
fn test_fast_shell_tracewise_why() {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let repo_dir = temp.path();
    std::fs::create_dir_all(repo_dir.join(".git/tracewise")).unwrap();
    let db_path = repo_dir.join(".git/tracewise/tracewise.db");
    let db = Database::open(&db_path).unwrap();
    drop(db);

    let orig_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(repo_dir).unwrap();

    // Invalid TRACEWISE_WHY: quality floor fails, no intent inserted
    std::env::set_var("TRACEWISE_WHY", "fix");
    let _ = fast_shell_begin(99991, vec!["echo".into(), "test".into(), ">".into(), "foo.txt".into()]);
    let db = Database::open(&db_path).unwrap();
    let intents = db.list_intents(None).unwrap();
    assert!(intents.is_empty(), "generic TRACEWISE_WHY should not create intent");
    drop(db);

    // Valid TRACEWISE_WHY: quality floor passes, intent inserted
    let valid_reason = "add comprehensive logging to the auth service";
    std::env::set_var("TRACEWISE_WHY", valid_reason);
    let _ = fast_shell_begin(99992, vec!["echo".into(), "test".into(), ">".into(), "foo.txt".into()]);
    let db = Database::open(&db_path).unwrap();
    let intents = db.list_intents(None).unwrap();
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].why, valid_reason);
    assert!(intents[0].paths.contains(&"foo.txt".to_string()));
    drop(db);

    std::env::remove_var("TRACEWISE_WHY");
    std::env::set_current_dir(orig_dir).unwrap();
}

#[test]
fn test_fast_shell_enforce_mode() {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let repo_dir = temp.path();
    std::fs::create_dir_all(repo_dir.join(".git/tracewise")).unwrap();
    let db_path = repo_dir.join(".git/tracewise/tracewise.db");
    let db = Database::open(&db_path).unwrap();
    drop(db);

    let orig_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(repo_dir).unwrap();

    std::env::set_var("TRACEWISE_REASONS", "enforce");
    std::env::set_var("TRACEWISE_ACTOR", "agent:codex");

    let res = fast_shell_begin(99993, vec!["echo".into(), "test".into(), ">".into(), "uncovered.txt".into()]);
    assert!(res.is_err(), "uncovered path in enforce mode must return Err");

    let db = Database::open(&db_path).unwrap();
    let expires = (chrono::Utc::now() + chrono::Duration::hours(4)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let intent = Intent {
        id: ulid::Ulid::new().to_string(),
        kind: "intent".to_string(),
        actor: Actor { kind: "agent".to_string(), name: Some("codex".to_string()), session: None },
        paths: vec!["uncovered.txt".to_string()],
        why: "add test coverage for enforce mode".to_string(),
        task: None,
        created: "2026-10-23T10:00:00.000Z".to_string(),
        expires,
        ended: 0,
    };
    db.insert_intent(&intent).unwrap();
    drop(db);

    let res2 = fast_shell_begin(99994, vec!["echo".into(), "test".into(), ">".into(), "uncovered.txt".into()]);
    assert!(res2.is_ok(), "covered path in enforce mode must succeed");

    std::env::remove_var("TRACEWISE_REASONS");
    std::env::remove_var("TRACEWISE_ACTOR");
    std::env::set_current_dir(orig_dir).unwrap();
}

#[test]
fn test_fast_shell_warn_mode() {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let repo_dir = temp.path();
    std::fs::create_dir_all(repo_dir.join(".git/tracewise")).unwrap();
    let db_path = repo_dir.join(".git/tracewise/tracewise.db");
    let db = Database::open(&db_path).unwrap();
    drop(db);

    let orig_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(repo_dir).unwrap();

    std::env::set_var("TRACEWISE_REASONS", "warn");
    std::env::set_var("TRACEWISE_ACTOR", "agent:codex");

    let _ = fast_shell_begin(99995, vec!["echo".into(), "test".into(), ">".into(), "warn.txt".into()]);
    std::fs::write(repo_dir.join("warn.txt"), "hello world\n").unwrap();
    fast_shell_end(99995, Some(0));

    let db = Database::open(&db_path).unwrap();
    let changes = db.list_changes(None, None).unwrap();
    assert!(!changes.is_empty());
    assert_eq!(changes[0].path, "warn.txt");
    assert!(changes[0].intent.is_none());

    std::env::remove_var("TRACEWISE_REASONS");
    std::env::remove_var("TRACEWISE_ACTOR");
    std::env::set_current_dir(orig_dir).unwrap();
}

#[test]
fn test_shell_commands_outside_git_repo() {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let outside = temp.path();
    let orig_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(outside).unwrap();

    assert!(try_fast_shell(&["tracewise".into(), "shell".into(), "begin".into(), "--pid".into(), "123".into(), "--".into(), "ls".into()]));
    assert!(try_fast_shell(&["tracewise".into(), "shell".into(), "end".into(), "--pid".into(), "123".into(), "--exit".into(), "0".into()]));
    assert!(try_fast_shell(&["tracewise".into(), "shell".into(), "preexec".into(), "--id".into(), "s1".into(), "--".into(), "ls".into()]));
    assert!(try_fast_shell(&[
        "tracewise".into(),
        "shell".into(),
        "precmd".into(),
        "--id".into(),
        "s1".into(),
        "--exit".into(),
        "0".into()
    ]));

    std::env::set_current_dir(orig_dir).unwrap();
}

#[test]
fn test_git_wrapper_install_target_resolution() {
    let tmp = tempfile::tempdir().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let self_exe = bin_dir.join("tracewise");
    fs::write(&self_exe, b"tracewise").unwrap();

    let target_bin = self_exe.parent().map(|p| p.join("tracewise-git")).filter(|p| p.is_file()).unwrap_or_else(|| self_exe.clone());
    assert_eq!(target_bin, self_exe);

    let tracewise_git = bin_dir.join("tracewise-git");
    fs::write(&tracewise_git, b"tracewise-git").unwrap();
    let target_bin = self_exe.parent().map(|p| p.join("tracewise-git")).filter(|p| p.is_file()).unwrap_or_else(|| self_exe.clone());
    assert_eq!(target_bin, tracewise_git);
}
