use super::export_destination;

use super::*;
use tempfile::tempdir;

#[test]
fn test_normalize_and_validate_path() {
    let dir = tempdir().unwrap();
    let wt = dir.path();

    // 1. Unsafe: "../x"
    assert_eq!(normalize_and_validate_path("../x", wt), None);

    // 2. Unsafe: "/etc/hosts"
    assert_eq!(normalize_and_validate_path("/etc/hosts", wt), None);

    // 3. Unsafe: ".git/hooks/pre-commit"
    assert_eq!(normalize_and_validate_path(".git/hooks/pre-commit", wt), None);

    // 4. Safe: absolute path inside the work tree
    let inside = wt.join("safe.txt");
    assert_eq!(normalize_and_validate_path(inside.to_str().unwrap(), wt), Some("safe.txt".to_string()));

    // Nested inside
    let nested = wt.join("subdir").join("nested.rs");
    assert_eq!(normalize_and_validate_path(nested.to_str().unwrap(), wt), Some("subdir/nested.rs".to_string()));

    // Safe relative paths
    assert_eq!(normalize_and_validate_path("safe.txt", wt), Some("safe.txt".to_string()));
    assert_eq!(normalize_and_validate_path("./safe.txt", wt), Some("safe.txt".to_string()));
    assert_eq!(normalize_and_validate_path("foo/../safe.txt", wt), Some("safe.txt".to_string()));
}

#[test]
fn test_unsafe_paths_in_hook_fail_open() {
    let _lock = crate::TEST_MUTEX.lock().unwrap();
    use crate::hook::{execute_hook, HookInput};
    let dir = tempdir().unwrap();
    let wt = dir.path();
    std::process::Command::new("git").arg("init").arg(wt).output().unwrap();

    let data_dir = wt.join(".tracewise").join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = data_dir.join("tracewise.db");
    let db = tracewise_core::db::Database::open(&db_path).unwrap();

    let orig_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(wt).unwrap();

    // 1. pre-edit with "../x"
    let input = HookInput {
        agent: Some("agent-1".to_string()),
        session: Some("sess-1".to_string()),
        paths: vec!["../x".to_string()],
        ..Default::default()
    };
    let res = execute_hook("pre-edit", input).unwrap();
    assert_eq!(res.decision, "allow");
    assert!(db.get_active_locks().unwrap().is_empty());

    // 2. pre-edit with "/etc/hosts"
    let input = HookInput {
        agent: Some("agent-1".to_string()),
        session: Some("sess-1".to_string()),
        paths: vec!["/etc/hosts".to_string()],
        ..Default::default()
    };
    let res = execute_hook("pre-edit", input).unwrap();
    assert_eq!(res.decision, "allow");
    assert!(db.get_active_locks().unwrap().is_empty());

    // 3. pre-edit with ".git/hooks/pre-commit"
    let input = HookInput {
        agent: Some("agent-1".to_string()),
        session: Some("sess-1".to_string()),
        paths: vec![".git/hooks/pre-commit".to_string()],
        ..Default::default()
    };
    let res = execute_hook("pre-edit", input).unwrap();
    assert_eq!(res.decision, "allow");
    assert!(db.get_active_locks().unwrap().is_empty());

    std::env::set_current_dir(orig_dir).unwrap();
}

#[test]
fn only_a_copy_out_of_the_repository_is_an_export() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo").canonicalize().unwrap_or_else(|_| tmp.path().join("repo"));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    let repo = repo.canonicalize().unwrap();
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let out = tmp.path().canonicalize().unwrap().join("out");
    assert_eq!(export_destination(&repo, &repo, "cp", &v(&["-R", "src", out.to_str().unwrap()])), Some(out.clone()));
    assert_eq!(export_destination(&repo, &repo, "cp", &v(&["-R", "src", "copy"])), None);
    assert_eq!(export_destination(&repo, &repo, "cp", &v(&["-R", "../x", "../y"])), None);
    assert_eq!(export_destination(&repo, &repo, "rm", &v(&["src", "x"])), None);
    // into another tracked repository: that one records it
    let other = tmp.path().canonicalize().unwrap().join("other");
    std::fs::create_dir_all(other.join(".git/tracewise")).unwrap();
    std::fs::write(other.join(".git/tracewise/tracewise.db"), b"").unwrap();
    assert_eq!(export_destination(&repo, &repo, "mv", &v(&["src", other.join("a").to_str().unwrap()])), None);
}
