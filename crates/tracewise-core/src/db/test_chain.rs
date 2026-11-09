use super::*;
use crate::model::{Actor, GitInfo, TimeRange};

fn chain_change(id: &str, path: &str) -> Change {
    Change {
        id: id.to_string(),
        time: TimeRange {
            start: format!("2026-10-23T10:00:00.{:03}Z", id.len() * 7 + path.len()),
            end: "2026-10-23T10:00:01.000Z".to_string(),
        },
        path: path.to_string(),
        op: "create".to_string(),
        old_path: None,
        hunks: Vec::new(),
        before: None,
        after: Some("blob1".to_string()),
        actor: Actor::agent("codex", "s1"),
        candidates: Vec::new(),
        confidence: "likely".to_string(),
        source: "shim".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "dev1".to_string(),
        git: GitInfo { branch: "main".to_string(), head: "commit1".to_string() },
        prev_hash: None,
        undoes: None,
    }
}

#[test]
fn the_chain_survives_edits_made_after_the_fact() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("tracewise.db")).unwrap();
    for (i, p) in ["a.txt", "b.txt", "c.txt"].iter().enumerate() {
        db.insert_change(&chain_change(&format!("id{i}"), p)).unwrap();
    }
    assert!(db.verify_hash_chain().unwrap().ok);
    // a reason added to an older change, an actor upgrade, a lock conflict: none may break the chain
    db.conn.execute("UPDATE changes SET intent = 'intent1' WHERE id = 'id0'", []).unwrap();
    db.conn
        .execute(
            "UPDATE changes SET actor_kind = 'human', actor_name = 'me', confidence = 'exact', lock_conflict = '{}' WHERE id = 'id1'",
            [],
        )
        .unwrap();
    let report = db.verify_hash_chain().unwrap();
    assert!(report.ok, "first bad: {:?}", report.first_bad);
    // but a changed path is still caught, and the first bad record is named
    db.conn.execute("UPDATE changes SET path = 'evil.txt' WHERE id = 'id1'", []).unwrap();
    let report = db.verify_hash_chain().unwrap();
    assert!(!report.ok);
    assert_eq!(report.first_bad.as_deref(), Some("id1"));
}

#[test]
fn deleting_the_newest_records_is_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tracewise.db");
    let db = Database::open(&path).unwrap();
    for (i, p) in ["a.txt", "b.txt", "c.txt"].iter().enumerate() {
        db.insert_change(&chain_change(&format!("id{i}"), p)).unwrap();
    }
    assert!(db.verify_hash_chain().unwrap().ok);
    db.conn.execute("DELETE FROM changes WHERE id = 'id2'", []).unwrap();
    let report = db.verify_hash_chain().unwrap();
    assert!(!report.ok && report.first_bad.as_deref() == Some("id2"));
}

#[test]
fn writers_in_different_processes_keep_one_chain() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("tracewise.db");
    drop(Database::open(&db_path).unwrap());
    std::thread::scope(|scope| {
        for t in 0..6 {
            let db_path = &db_path;
            scope.spawn(move || {
                let db = Database::open(db_path).unwrap();
                for i in 0..15 {
                    db.insert_change(&chain_change(&format!("w{t}-{i}"), &format!("f{t}-{i}.txt"))).unwrap();
                }
            });
        }
    });
    let db = Database::open(&db_path).unwrap();
    let report = db.verify_hash_chain().unwrap();
    assert_eq!(report.changes, 90);
    assert!(report.ok, "chain forked at {:?}", report.first_bad);
}
