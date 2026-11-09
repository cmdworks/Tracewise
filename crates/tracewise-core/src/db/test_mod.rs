use super::*;
use crate::model::{Actor, Change, CommandRecord, GitInfo, TimeRange};

#[test]
fn test_db_init_and_crud() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    let change = Change {
        id: "01JBZ6Q4K7X8V2N3M4P5Q6R7S8".to_string(),
        time: TimeRange::now(),
        path: "test.txt".to_string(),
        op: "create".to_string(),
        old_path: None,
        hunks: vec![],
        before: None,
        after: Some("abc".to_string()),
        actor: Actor::baseline(),
        candidates: vec![],
        confidence: "exact".to_string(),
        source: "baseline".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "dev-test".to_string(),
        git: GitInfo { branch: "main".to_string(), head: "12345678".to_string() },
        prev_hash: None,
        undoes: None,
    };

    db.insert_change(&change).unwrap();
    assert_eq!(db.count_changes().unwrap(), 1);

    let fetched = db.get_change("01JBZ6Q4K7X8V2N3M4P5Q6R7S8").unwrap().unwrap();
    assert_eq!(fetched.id, change.id);
    assert_eq!(fetched.actor.kind, "baseline");

    let list = db.list_changes(None, None).unwrap();
    assert_eq!(list.len(), 1);

    let filtered = db.list_changes(Some("other.txt"), None).unwrap();
    assert_eq!(filtered.len(), 0);
}

#[test]
fn test_db_gc_truncates_wal_and_vacuums_s10_s11() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("trail.db");
    let wal_path = temp.path().join("trail.db-wal");

    let db = Database::open(&db_path).unwrap();

    // Insert 500 expired commands (40 days old) with sizable text
    let old_time = (chrono::Utc::now() - chrono::Duration::days(40)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    for i in 0..500 {
        let cmd = CommandRecord {
            id: format!("cmd_old_{i}"),
            kind: "shell".to_string(),
            text: format!("command number {i} with payload padding {}", "z".repeat(500)),
            cwd: None,
            actor: Actor { kind: "human".to_string(), name: Some("asmat".to_string()), session: None },
            start: old_time.clone(),
            end: old_time.clone(),
            exit_code: 0,
            changes: vec![],
        };
        db.insert_command(&cmd).unwrap();
    }

    // Insert 5 recent commands
    let new_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    for i in 0..5 {
        let cmd = CommandRecord {
            id: format!("cmd_new_{i}"),
            kind: "shell".to_string(),
            text: format!("new command {i}"),
            cwd: None,
            actor: Actor { kind: "human".to_string(), name: Some("asmat".to_string()), session: None },
            start: new_time.clone(),
            end: new_time.clone(),
            exit_code: 0,
            changes: vec![],
        };
        db.insert_command(&cmd).unwrap();
    }

    assert_eq!(db.list_commands().unwrap().len(), 505);

    // Verify WAL file has accumulated writes (> 0 bytes)
    if wal_path.exists() {
        let wal_meta = std::fs::metadata(&wal_path).unwrap();
        assert!(wal_meta.len() > 0, "WAL file should have pending writes before gc");
    }

    // Execute GC
    let report = db.gc(None).unwrap();
    assert_eq!(report.commands_pruned, 500);
    assert_eq!(db.list_commands().unwrap().len(), 5);

    // Verify WAL file was truncated to 0 bytes
    if wal_path.exists() {
        let wal_meta = std::fs::metadata(&wal_path).unwrap();
        assert_eq!(wal_meta.len(), 0, "WAL file must be truncated to 0 bytes post-gc (S10)");
    }

    // Verify database file is compacted
    let db_meta = std::fs::metadata(&db_path).unwrap();
    assert!(db_meta.len() < 200_000, "Compacted database size should be small after VACUUM: {} bytes", db_meta.len());
}

#[test]
fn gc_keeps_recent_commands() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let recent = CommandRecord {
        id: "git_ro_recent".to_string(),
        kind: "git".to_string(),
        text: "git status".to_string(),
        cwd: None,
        actor: Actor { kind: "human".to_string(), name: Some("asmat".to_string()), session: None },
        start: now.clone(),
        end: now,
        exit_code: 0,
        changes: vec![],
    };
    db.insert_command(&recent).unwrap();
    db.gc(None).unwrap();
    assert_eq!(db.list_commands().unwrap().len(), 1);
}

#[test]
fn a_new_history_gets_the_schema_and_an_old_one_is_refused_not_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trail.db");
    drop(Database::open(&path).unwrap());
    let conn = rusqlite::Connection::open(&path).unwrap();
    let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_eq!(v, LATEST_SCHEMA_VERSION);
    conn.pragma_update(None, "user_version", 9).unwrap();
    drop(conn);
    let err = Database::open(&path).err().expect("an old schema must be refused").to_string();
    assert!(err.contains("does not migrate") && err.contains("trail init"), "{err}");
    let conn = rusqlite::Connection::open(&path).unwrap();
    let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_eq!(v, 9, "the refused history is left as it was");
}
