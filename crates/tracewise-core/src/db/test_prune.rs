use super::*;
use crate::model::{Actor, Change, GitInfo, TimeRange};

fn change(n: usize, path: &str) -> Change {
    Change {
        id: format!("id{n:03}"),
        time: TimeRange { start: format!("2026-10-23T10:00:{n:02}.000Z"), end: format!("2026-10-23T10:00:{n:02}.500Z") },
        path: path.to_string(),
        op: "create".to_string(),
        old_path: None,
        hunks: Vec::new(),
        before: None,
        after: Some(format!("blob{n}")),
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

/// A database with one record per path, in order.
fn db_with(dir: &std::path::Path, paths: &[&str]) -> Database {
    let db = Database::open(&dir.join("trail.db")).unwrap();
    for (n, p) in paths.iter().enumerate() {
        db.insert_change(&change(n, p)).unwrap();
    }
    db
}

fn ids(list: &[usize]) -> HashSet<String> {
    list.iter().map(|n| format!("id{n:03}")).collect()
}

fn left(db: &Database) -> Vec<String> {
    db.all_changes_in_insert_order().unwrap().into_iter().map(|c| c.id).collect()
}

#[test]
fn a_pruned_middle_run_still_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "target/x", "target/y", "b", "c"]);
    let report = db.prune_changes(&ids(&[1, 2]), "me", "ignored", true, false).unwrap();
    assert_eq!((report.removed, report.gaps), (2, 1));
    assert_eq!(report.folders.get("target"), Some(&2));
    assert_eq!(left(&db), ["id000", "id003", "id004"]);
    let v = db.verify_hash_chain().unwrap();
    assert!(v.ok, "first bad: {:?}", v.first_bad);
    assert_eq!(db.prune_events().unwrap().len(), 1);
}

#[test]
fn the_start_and_two_separate_runs_verify() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["t/a", "t/b", "keep1", "t/c", "keep2", "t/d", "keep3"]);
    let report = db.prune_changes(&ids(&[0, 1, 3, 5]), "me", "ignored", true, false).unwrap();
    assert_eq!((report.removed, report.gaps), (4, 3));
    assert!(db.verify_hash_chain().unwrap().ok);
}

#[test]
fn pruning_again_across_an_old_gap_still_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "t/1", "b", "t/2", "c", "d"]);
    db.prune_changes(&ids(&[1, 3]), "me", "first", true, false).unwrap();
    // the survivors between the old gaps now go too, merging everything into one run
    db.prune_changes(&ids(&[2]), "me", "second", true, false).unwrap();
    assert_eq!(left(&db), ["id000", "id004", "id005"]);
    assert!(db.verify_hash_chain().unwrap().ok);
    // and the very first record, then the next ones
    db.prune_changes(&ids(&[0]), "me", "third", true, false).unwrap();
    assert!(db.verify_hash_chain().unwrap().ok);
}

#[test]
fn the_newest_record_is_never_pruned() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "b", "c"]);
    let report = db.prune_changes(&ids(&[1, 2]), "me", "all", true, false).unwrap();
    assert_eq!(report.removed, 1);
    assert_eq!(left(&db), ["id000", "id002"]);
    assert!(db.verify_hash_chain().unwrap().ok);
}

#[test]
fn a_dry_run_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "t/1", "b"]);
    let report = db.prune_changes(&ids(&[1]), "me", "ignored", true, true).unwrap();
    assert_eq!(report.removed, 1);
    assert_eq!(left(&db).len(), 3);
    assert!(db.prune_events().unwrap().is_empty());
}

#[test]
fn a_tampered_survivor_or_gap_still_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "t/1", "b", "c"]);
    db.prune_changes(&ids(&[1]), "me", "ignored", true, false).unwrap();
    assert!(db.verify_hash_chain().unwrap().ok);
    db.conn.execute("UPDATE chain_meta SET value = 'forged' WHERE key LIKE 'gap:%'", []).unwrap();
    assert!(!db.verify_hash_chain().unwrap().ok);

    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "t/1", "b", "c"]);
    db.prune_changes(&ids(&[1]), "me", "ignored", true, false).unwrap();
    db.conn.execute("UPDATE changes SET path = 'evil' WHERE id = 'id002'", []).unwrap();
    assert!(!db.verify_hash_chain().unwrap().ok);
}

#[test]
fn state_and_references_of_a_forgotten_path_go_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "t/1", "b"]);
    db.conn.execute("INSERT INTO files (path, blob, size, mtime_ns) VALUES ('t/1', 'x', 1, 1)", []).unwrap();
    db.conn.execute("INSERT INTO file_changes (path, change_seq, change_id) VALUES ('t/1', 1, 'id001')", []).unwrap();
    db.conn.execute("INSERT INTO undo_records (id, undoes) VALUES ('u1', 'id001')", []).unwrap();
    db.conn
        .execute("INSERT INTO commands (id, kind, text, actor, start, end, exit_code, changes) VALUES ('c1', 'run', 'x', 'me', 's', 'e', 0, '[\"id001\",\"id000\"]')", [])
        .unwrap();
    db.prune_changes(&ids(&[1]), "me", "ignored", true, false).unwrap();
    let count = |sql: &str| db.conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM files WHERE path = 't/1'"), 0);
    assert_eq!(count("SELECT COUNT(*) FROM file_changes WHERE path = 't/1'"), 0);
    assert_eq!(count("SELECT COUNT(*) FROM undo_records"), 0);
    let cmd: String = db.conn.query_row("SELECT changes FROM commands WHERE id = 'c1'", [], |r| r.get(0)).unwrap();
    assert_eq!(cmd, "[\"id000\"]");
}

#[test]
fn state_stays_when_the_path_is_not_forgotten() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "keep", "b"]);
    db.conn.execute("INSERT INTO files (path, blob, size, mtime_ns) VALUES ('keep', 'x', 1, 1)", []).unwrap();
    db.prune_changes(&ids(&[1]), "me", "--path keep", false, false).unwrap();
    let n: i64 = db.conn.query_row("SELECT COUNT(*) FROM files WHERE path = 'keep'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn only_pushed_ids_are_listed_as_pushed() {
    let dir = tempfile::tempdir().unwrap();
    let db = db_with(dir.path(), &["a", "b"]);
    db.mark_changes_pushed(&["id000".to_string()]).unwrap();
    assert_eq!(db.pushed_change_ids().unwrap(), ids(&[0]));
}

#[test]
fn rename_onto_a_path_with_rows_replaces_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = db_with(dir.path(), &["a.txt", "b.txt"]);
    let mut map = crate::linemap::LineMap::new();
    map.push_run(1, 2);
    db.set_file_line_map("a.txt", &map, &[(1, "id000")]).unwrap();
    db.set_file_line_map("b.txt", &map, &[(1, "id001")]).unwrap();
    db.rename_file_lines("a.txt", "b.txt").unwrap();
    assert!(db.get_file_line_map("a.txt").unwrap().is_none());
    assert!(db.get_file_line_map("b.txt").unwrap().is_some());
}
