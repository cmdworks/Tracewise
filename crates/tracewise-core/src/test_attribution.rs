use super::*;
use crate::model::{Change, GitInfo, Intent, TimeRange};

#[test]
fn test_attribute_window_reads_only_returns_empty() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("tracewise.db")).unwrap();

    let window = Window {
        id: "win1".to_string(),
        kind: "shell".to_string(),
        actor: Actor { kind: "human".to_string(), name: Some("alice".to_string()), session: None },
        paths: vec!["file.txt".to_string()],
        cwd: None,
        start: "2026-10-25T10:00:00.000Z".to_string(),
        end: None,
        command: Some("cat file.txt".to_string()),
    };

    let result = attribute_window(&db, &window, &window.actor, true, true).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_attribute_window_claims_unknown_and_links_intent() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("tracewise.db")).unwrap();

    let window_start = "2026-10-25T10:00:00.000Z";
    let agent = Actor::agent("claude", "s1");

    // Insert intent covering file.txt
    let intent = Intent {
        id: "int1".to_string(),
        actor: agent.clone(),
        paths: vec!["src/*.rs".to_string()],
        why: "refactor".to_string(),
        task: None,
        created: window_start.to_string(),
        expires: "2099-01-01T00:00:00.000Z".to_string(),
        kind: "intent".to_string(),
        ended: 0,
    };
    db.insert_intent(&intent).unwrap();

    // Insert unknown change during the window
    let change = Change {
        id: "chg1".to_string(),
        time: TimeRange { start: "2026-10-25T10:00:01.000Z".to_string(), end: "2026-10-25T10:00:02.000Z".to_string() },
        path: "src/main.rs".to_string(),
        op: "modify".to_string(),
        old_path: None,
        hunks: vec![],
        before: None,
        after: Some("blob1".to_string()),
        actor: Actor::unknown(),
        candidates: vec![],
        confidence: "unknown".to_string(),
        source: "scan".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "dev1".to_string(),
        git: GitInfo { branch: "main".to_string(), head: "commit1".to_string() },
        prev_hash: None,
        undoes: None,
    };
    db.insert_change(&change).unwrap();

    let window = Window {
        id: "win2".to_string(),
        kind: "agent_command".to_string(),
        actor: agent.clone(),
        paths: vec!["src/main.rs".to_string()],
        cwd: None,
        start: window_start.to_string(),
        end: None,
        command: Some("cargo build".to_string()),
    };

    let attributed = attribute_window(&db, &window, &agent, true, false).unwrap();
    assert_eq!(attributed, vec!["chg1"]);

    // Verify change was upgraded and linked to intent
    let ch = db.get_change("chg1").unwrap().unwrap();
    assert_eq!(ch.actor, agent);
    assert_eq!(ch.confidence, "exact");
    assert_eq!(ch.intent.as_deref(), Some("int1"));
}

#[test]
fn a_bulk_edit_over_one_hundred_files_is_all_attributed() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("tracewise.db")).unwrap();
    let agent = Actor::agent("claude", "s1");
    let start = "2026-10-25T10:00:00.000Z";
    db.insert_intent(&Intent {
        id: "int1".to_string(),
        actor: agent.clone(),
        paths: vec!["src/*.rs".to_string()],
        why: "rename everywhere".to_string(),
        task: None,
        created: start.to_string(),
        expires: "2099-01-01T00:00:00.000Z".to_string(),
        kind: "intent".to_string(),
        ended: 0,
    })
    .unwrap();
    for i in 0..150 {
        db.insert_change(&Change {
            id: format!("chg{i:03}"),
            time: TimeRange { start: "2026-10-25T10:00:01.000Z".to_string(), end: "2026-10-25T10:00:02.000Z".to_string() },
            path: format!("src/f{i}.rs"),
            op: "modify".to_string(),
            old_path: None,
            hunks: vec![],
            before: None,
            after: Some("blob1".to_string()),
            actor: agent.clone(),
            candidates: vec![],
            confidence: "exact".to_string(),
            source: "hook".to_string(),
            command: None,
            intent: None,
            lock_conflict: None,
            device: "dev1".to_string(),
            git: GitInfo { branch: "main".to_string(), head: "commit1".to_string() },
            prev_hash: None,
            undoes: None,
        })
        .unwrap();
    }
    let window = Window {
        id: "win3".to_string(),
        kind: "agent_command".to_string(),
        actor: agent.clone(),
        paths: vec!["src/*.rs".to_string()],
        cwd: None,
        start: start.to_string(),
        end: None,
        command: Some("sed -i".to_string()),
    };
    let attributed = attribute_window(&db, &window, &agent, true, false).unwrap();
    assert_eq!(attributed.len(), 150);
    let linked = (0..150).filter(|i| db.get_change(&format!("chg{i:03}")).unwrap().unwrap().intent.as_deref() == Some("int1")).count();
    assert_eq!(linked, 150, "every file gets the covering reason");
}

#[test]
fn a_failed_write_is_an_error_not_a_success() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("tracewise.db")).unwrap();
    let agent = Actor::agent("claude", "s1");
    let change = |id: &str, actor: Actor| Change {
        id: id.to_string(),
        time: TimeRange { start: "2026-10-25T10:00:01.000Z".to_string(), end: "2026-10-25T10:00:02.000Z".to_string() },
        path: "src/main.rs".to_string(),
        op: "modify".to_string(),
        old_path: None,
        hunks: vec![],
        before: None,
        after: Some("blob1".to_string()),
        actor,
        candidates: vec![],
        confidence: "unknown".to_string(),
        source: "scan".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "dev1".to_string(),
        git: GitInfo { branch: "main".to_string(), head: "commit1".to_string() },
        prev_hash: None,
        undoes: None,
    };
    db.insert_change(&change("chg1", Actor::unknown())).unwrap();
    // every later update of a change fails, as a locked or damaged database would
    db.conn.execute_batch("CREATE TRIGGER no_update BEFORE UPDATE ON changes BEGIN SELECT RAISE(ABORT, 'disk I/O error'); END;").unwrap();
    let window = Window {
        id: "w".to_string(),
        kind: "agent_command".to_string(),
        actor: agent.clone(),
        paths: vec!["src/main.rs".to_string()],
        cwd: None,
        start: "2026-10-25T10:00:00.000Z".to_string(),
        end: None,
        command: None,
    };
    assert!(attribute_window(&db, &window, &agent, true, false).is_err());
}
