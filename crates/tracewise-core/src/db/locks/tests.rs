use super::*;

#[test]
fn test_locks_crud() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    let claude = Actor::agent("claude", "s1");
    let codex = Actor::agent("codex", "c1");

    // Acquire lock
    let conflict = db.acquire_lock("f.txt", &claude, None, 10).unwrap();
    assert!(conflict.is_none());

    // List locks
    let active = db.get_active_locks().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].path, "f.txt");
    assert_eq!(active[0].holder.name.as_deref(), Some("claude"));

    // Second agent denied
    let conflict2 = db.acquire_lock("f.txt", &codex, None, 10).unwrap();
    assert!(conflict2.is_some());
    assert_eq!(conflict2.unwrap().holder.name.as_deref(), Some("claude"));

    // Holder renews
    let conflict3 = db.acquire_lock("f.txt", &claude, None, 10).unwrap();
    assert!(conflict3.is_none());

    // Release
    assert!(db.release_lock_path("f.txt").unwrap());
    assert_eq!(db.count_active_locks().unwrap(), 0);

    // Codex can now acquire
    let conflict4 = db.acquire_lock("f.txt", &codex, None, 10).unwrap();
    assert!(conflict4.is_none());

    // Release session locks
    db.release_session_locks("c1").unwrap();
    assert_eq!(db.count_active_locks().unwrap(), 0);
}

#[test]
fn eight_processes_racing_for_one_file_get_exactly_one_lock() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("trail.db");
    drop(Database::open(&db_path).unwrap()); // create the schema once
    let granted = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for i in 0..8 {
            let (db_path, granted) = (&db_path, &granted);
            scope.spawn(move || {
                let db = Database::open(db_path).unwrap();
                let holder = Actor { kind: "agent".to_string(), name: Some(format!("agent{i}")), session: Some(format!("s{i}")) };
                if db.acquire_lock("race.txt", &holder, None, 60).unwrap().is_none() {
                    granted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            });
        }
    });
    assert_eq!(granted.load(std::sync::atomic::Ordering::SeqCst), 1);
}
