//! Tests for daemon and capture.
use super::*;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_daemon_sweep_stuck_flags_after_crash() {
    let _lock = crate::TEST_MUTEX.lock().unwrap();
    let dir = tempdir().unwrap();
    let wt = dir.path();

    std::process::Command::new("git").arg("init").arg(wt).output().unwrap();

    let test_file = wt.join("test.txt");
    std::fs::write(&test_file, b"initial content").unwrap();

    let data_dir = wt.join(".tracewise").join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = data_dir.join("tracewise.db");
    let db = Database::open(&db_path).unwrap();

    // Spawn a child process to act as the holder
    let mut child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
    let holder_pid = child.id();
    let holder_actor = Actor::agent("test-agent", &format!("sess-{}", holder_pid));

    // Take lock with 1-second TTL
    let conflict = db.acquire_lock("test.txt", &holder_actor, None, 1).unwrap();
    assert!(conflict.is_none());

    // Kill holder with SIGKILL (kill -9)
    let _ = tracewise_platform::signal::kill_process(holder_pid as i32);
    let _ = child.wait();

    // Start daemon loop with fast sweep (TRACEWISE_SWEEP_INTERVAL_SECS=1)
    unsafe {
        std::env::set_var("TRACEWISE_SWEEP_INTERVAL_SECS", "1");
    }
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let wt_clone = wt.to_path_buf();
    let sweep_wt = wt.to_path_buf();
    let data_dir_clone = data_dir.clone();

    let daemon_thread = std::thread::spawn(move || {
        let _ = tracewise_watch::watch_loop(
            &wt_clone,
            50,
            r,
            |_| true,
            move || {
                let db_path = data_dir_clone.join("tracewise.db");
                if let Ok(db) = Database::open_hot(&db_path) {
                    let _ = db.clean_expired_locks(&sweep_wt);
                }
            },
        );
    });

    // Wait for expiry and daemon sweep (TTL 1s + sweep 1s)
    let mut ok = false;
    for _ in 0..15 {
        std::thread::sleep(Duration::from_millis(200));
        if let Ok(mut f) = std::fs::OpenOptions::new().write(true).truncate(true).open(&test_file) {
            if f.write_all(b"writable again").is_ok() {
                ok = true;
                break;
            }
        }
    }
    assert!(ok, "file must be writable again after daemon sweeps expired lock");

    // Stop daemon
    running.store(false, Ordering::SeqCst);
    let _ = std::fs::write(wt.join("wake.txt"), b"wake");
    let _ = daemon_thread.join();
    unsafe {
        std::env::remove_var("TRACEWISE_SWEEP_INTERVAL_SECS");
    }
}
