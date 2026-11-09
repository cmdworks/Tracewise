use super::*;

#[test]
fn test_list_pending_changes() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    let make_change = |id: &str, path: &str, source: &str, intent: Option<&str>| Change {
        id: id.to_string(),
        time: TimeRange { start: "2026-10-23T10:00:00.000Z".to_string(), end: "2026-10-23T10:00:01.000Z".to_string() },
        path: path.to_string(),
        op: "create".to_string(),
        old_path: None,
        hunks: Vec::new(),
        before: None,
        after: Some("blob1".to_string()),
        actor: Actor::agent("codex", "s1"),
        candidates: Vec::new(),
        confidence: "exact".to_string(),
        source: source.to_string(),
        command: None,
        intent: intent.map(|s| s.to_string()),
        lock_conflict: None,
        device: "dev1".to_string(),
        git: GitInfo { branch: "main".to_string(), head: "commit1".to_string() },
        prev_hash: None,
        undoes: None,
    };

    // Baseline change (should be excluded)
    db.insert_change(&make_change("c1", "base.txt", "baseline", None)).unwrap();
    // Change with intent (should be excluded)
    db.insert_change(&make_change("c2", "noted.txt", "shell", Some("intent1"))).unwrap();
    // Pending change without intent
    db.insert_change(&make_change("c3", "pending.txt", "shell", None)).unwrap();

    let pending = db.list_pending_changes().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "c3");
    assert_eq!(pending[0].path, "pending.txt");
    assert_eq!(pending[0].actor.name.as_deref(), Some("codex"));
}

#[test]
fn test_shell_pending_count() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    assert_eq!(db.get_shell_pending_count("tty1").unwrap(), None);
    db.set_shell_pending_count("tty1", 3).unwrap();
    assert_eq!(db.get_shell_pending_count("tty1").unwrap(), Some(3));
    db.set_shell_pending_count("tty1", 5).unwrap();
    assert_eq!(db.get_shell_pending_count("tty1").unwrap(), Some(5));
}

#[test]
fn test_insert_and_get_change_undoes() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    let change = Change {
        id: "01JUNDO0000000000000000001".to_string(),
        time: TimeRange { start: "2026-10-26T00:00:00.000Z".to_string(), end: "2026-10-26T00:00:01.000Z".to_string() },
        path: "f.txt".to_string(),
        op: "undo".to_string(),
        old_path: None,
        hunks: Vec::new(),
        before: Some("blob_before".to_string()),
        after: Some("blob_after".to_string()),
        actor: Actor { kind: "human".to_string(), name: Some("alice".to_string()), session: None },
        candidates: Vec::new(),
        confidence: "exact".to_string(),
        source: "undo".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "dev1".to_string(),
        git: GitInfo { branch: "main".to_string(), head: "commit1".to_string() },
        prev_hash: None,
        undoes: Some(vec!["01JORIG0000000000000000001".to_string()]),
    };

    db.insert_change(&change).unwrap();

    let loaded = db.get_change(&change.id).unwrap().unwrap();
    assert_eq!(loaded.op, "undo");
    assert_eq!(loaded.undoes, Some(vec!["01JORIG0000000000000000001".to_string()]));

    let listed = db.list_changes(None, None).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].undoes, Some(vec!["01JORIG0000000000000000001".to_string()]));
}

#[test]
fn a_limited_path_history_never_exceeds_the_limit_for_the_baseline_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();
    let make = |id: &str, path: &str, op: &str| Change {
        id: id.to_string(),
        time: TimeRange { start: format!("2026-10-23T10:00:0{}.000Z", &id[1..]), end: format!("2026-10-23T10:00:0{}.500Z", &id[1..]) },
        path: path.to_string(),
        op: op.to_string(),
        old_path: None,
        hunks: Vec::new(),
        before: None,
        after: Some("blob1".to_string()),
        actor: Actor::agent("codex", "s1"),
        candidates: Vec::new(),
        confidence: "exact".to_string(),
        source: "shell".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "dev1".to_string(),
        git: GitInfo { branch: "main".to_string(), head: "commit1".to_string() },
        prev_hash: None,
        undoes: None,
    };
    db.insert_change(&make("c1", "a.txt", "create")).unwrap();
    db.insert_change(&make("c2", "a.txt", "modify")).unwrap();
    db.insert_change(&make("c0", "z.txt", "baseline")).unwrap();
    db.set_file(&crate::model::FileRecord { path: "a.txt".into(), blob: "blob1".into(), size: 1, mtime_ns: 0 }).unwrap();
    assert_eq!(db.list_changes(Some("a.txt"), Some(1)).unwrap().len(), 1, "the create is older than the limit, not missing");

    // a file with no create of its own: the baseline stands in for it when everything fits
    db.insert_change(&make("c3", "b.txt", "modify")).unwrap();
    db.set_file(&crate::model::FileRecord { path: "b.txt".into(), blob: "blob1".into(), size: 1, mtime_ns: 0 }).unwrap();
    let rows = db.list_changes(Some("b.txt"), Some(5)).unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
}
