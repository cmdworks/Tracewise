use super::*;
use crate::model::{Change, GitInfo, TimeRange};

#[test]
fn test_windows_and_upgrade() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    let window = Window {
        id: "01JBZ6Q4K7X8V2N3M4P5Q6R7S9".to_string(),
        kind: "agent_edit".to_string(),
        actor: Actor::agent("claude", "s1"),
        paths: vec!["src/main.rs".to_string()],
        cwd: None,
        start: "2026-10-23T00:00:00.000Z".to_string(),
        end: None,
        command: None,
    };

    db.open_window(&window).unwrap();

    let closed = db.close_latest_window("claude", "s1", "agent_edit", "2026-10-23T00:01:00.000Z").unwrap();
    assert!(closed.is_some());
    let closed_w = closed.unwrap();
    assert_eq!(closed_w.id, window.id);
    assert_eq!(closed_w.end, Some("2026-10-23T00:01:00.000Z".to_string()));

    // Test change upgrade
    let change = Change {
        id: "01JBZ6Q4K7X8V2N3M4P5Q6R7SA".to_string(),
        time: TimeRange { start: "2026-10-23T00:00:30.000Z".to_string(), end: "2026-10-23T00:00:30.000Z".to_string() },
        path: "src/main.rs".to_string(),
        op: "modify".to_string(),
        old_path: None,
        hunks: vec![],
        before: None,
        after: None,
        actor: Actor::unknown(),
        candidates: vec![],
        confidence: "unknown".to_string(),
        source: "scan".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "dev-test".to_string(),
        git: GitInfo { branch: "main".to_string(), head: "12345678".to_string() },
        prev_hash: None,
        undoes: None,
    };
    db.insert_change(&change).unwrap();

    let unknown = db.find_unknown_changes_for_path_since("src/main.rs", "2026-10-23T00:00:00.000Z").unwrap();
    assert_eq!(unknown.len(), 1);

    db.upgrade_change_actor(&change.id, &Actor::agent("claude", "s1"), "exact", "hook").unwrap();

    let updated = db.get_change(&change.id).unwrap().unwrap();
    assert_eq!(updated.actor, Actor::agent("claude", "s1"));
    assert_eq!(updated.confidence, "exact");
    assert_eq!(updated.source, "hook");
}
