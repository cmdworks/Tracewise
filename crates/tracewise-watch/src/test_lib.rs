use super::*;

/// A batch covers a path when it names it, or is empty: an empty batch means "events may be lost, scan the whole tree"
/// (macOS drops events under heavy file activity), which also covers the path.
fn covers(batch: &[PathBuf], path: &Path) -> bool {
    batch.is_empty() || batch.contains(&path.to_path_buf())
}

/// Event delivery can be slow on a busy machine: wait this long for each expected batch.
const PATIENCE: Duration = Duration::from_secs(10);

#[test]
fn test_watch_subdirectory() {
    let temp_raw = std::env::temp_dir().join(format!("watch_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_raw);
    std::fs::create_dir_all(&temp_raw).unwrap();
    let temp = temp_raw.canonicalize().unwrap();
    let src_dir = temp.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let file_path = src_dir.join("test.txt");
    std::fs::write(&file_path, b"initial").unwrap();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let (tx, rx) = std::sync::mpsc::channel();

    let temp_path = temp.clone();
    let handle = std::thread::spawn(move || {
        let _ = watch_loop(
            &temp_path,
            50,
            r,
            move |paths| {
                let _ = tx.send(paths.to_vec());
                true
            },
            || {},
        );
    });

    std::thread::sleep(Duration::from_millis(500));
    // FSEvents sometimes starts late: write again until the first event arrives (3 tries, 3 s each)
    let mut received = Err(std::sync::mpsc::RecvTimeoutError::Timeout);
    for attempt in 0..3 {
        std::fs::write(&file_path, format!("modified {attempt}")).unwrap();
        received = rx.recv_timeout(PATIENCE);
        if received.is_ok() {
            break;
        }
    }
    running.store(false, Ordering::SeqCst);
    let _ = std::fs::write(&file_path, b"stop");
    let _ = handle.join();
    let _ = std::fs::remove_dir_all(&temp);

    assert!(received.is_ok(), "should receive change event for file in subdirectory");
    let paths = received.unwrap();
    assert!(covers(&paths, &file_path), "the batch names the file or asks for a whole-tree scan: {paths:?}");
}

#[test]
fn test_watch_loop_immediate_stop() {
    let temp = std::env::temp_dir();
    let running = Arc::new(AtomicBool::new(false));
    let res = watch_loop(&temp, 100, running, |_| true, || {});
    assert!(res.is_ok());
}

#[test]
fn a_failed_recording_is_retried_without_another_edit() {
    let temp_raw = std::env::temp_dir().join(format!("watch_retry_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_raw);
    std::fs::create_dir_all(&temp_raw).unwrap();
    let temp = temp_raw.canonicalize().unwrap();
    let file_path = temp.join("a.txt");
    std::fs::write(&file_path, b"initial").unwrap();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let temp_path = temp.clone();
    let handle = std::thread::spawn(move || {
        let mut calls = 0;
        let _ = watch_loop(
            &temp_path,
            50,
            r,
            move |paths| {
                calls += 1;
                let _ = tx.send((calls, paths.to_vec()));
                calls >= 3 // the first two scans "fail"
            },
            || {},
        );
    });

    std::thread::sleep(Duration::from_millis(500));
    // one edit only (written again only until the first event arrives: FSEvents may start late)
    let mut seen = Vec::new();
    for attempt in 0..3 {
        std::fs::write(&file_path, format!("modified {attempt}")).unwrap();
        if let Ok(first) = rx.recv_timeout(PATIENCE) {
            seen.push(first);
            break;
        }
    }
    while let Ok(next) = rx.recv_timeout(PATIENCE) {
        seen.push(next);
        if seen.len() >= 3 {
            break;
        }
    }
    running.store(false, Ordering::SeqCst);
    let _ = std::fs::write(&file_path, b"stop");
    let _ = handle.join();
    let _ = std::fs::remove_dir_all(&temp);

    assert!(seen.len() >= 3, "the failed batch must be offered again: {seen:?}");
    assert!(seen.iter().all(|(_, paths)| covers(paths, &file_path)), "the same path each time (or a whole-tree scan): {seen:?}");
}

#[test]
fn a_burst_over_the_cap_becomes_one_whole_tree_scan() {
    let temp = std::env::temp_dir().join(format!("watch_cap_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let temp = temp.canonicalize().unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let t = temp.clone();
    let handle = std::thread::spawn(move || {
        let _ = watch_loop_capped(
            &t,
            200,
            r,
            2,
            |_| false,
            move |paths| {
                let _ = tx.send(paths.len());
                true
            },
            || {},
        );
    });
    std::thread::sleep(Duration::from_millis(500));
    let mut sizes = Vec::new();
    for attempt in 0..3 {
        for i in 0..20 {
            std::fs::write(temp.join(format!("f{attempt}_{i}")), b"x").unwrap();
        }
        if let Ok(first) = rx.recv_timeout(PATIENCE) {
            sizes.push(first);
            while let Ok(n) = rx.recv_timeout(Duration::from_secs(1)) {
                sizes.push(n);
            }
        }
        if !sizes.is_empty() {
            break;
        }
    }
    running.store(false, Ordering::SeqCst);
    let _ = std::fs::write(temp.join("stop"), b"stop");
    let _ = handle.join();
    let _ = std::fs::remove_dir_all(&temp);
    assert!(sizes.contains(&0), "over the cap, one whole-tree scan (empty slice) is due: {sizes:?}");
    assert!(sizes.iter().all(|n| *n <= 2), "never more paths than the cap at once: {sizes:?}");
}

#[test]
fn a_skipped_folder_never_reaches_the_queue() {
    let temp = std::env::temp_dir().join(format!("watch_skip_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(temp.join("target")).unwrap();
    let temp = temp.canonicalize().unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let t = temp.clone();
    let handle = std::thread::spawn(move || {
        let _ = watch_loop_filtered(
            &t,
            100,
            r,
            |p| p.components().any(|c| c.as_os_str() == "target"),
            move |paths| {
                let _ = tx.send(paths.to_vec());
                true
            },
            || {},
        );
    });
    std::thread::sleep(Duration::from_millis(500));
    let kept = temp.join("kept.txt");
    let mut seen: Vec<Vec<PathBuf>> = Vec::new();
    for attempt in 0..3 {
        for i in 0..50 {
            std::fs::write(temp.join("target").join(format!("o{i}")), format!("{attempt}")).unwrap();
        }
        std::fs::write(&kept, format!("{attempt}")).unwrap();
        while let Ok(b) = rx.recv_timeout(PATIENCE) {
            let done = covers(&b, &kept);
            seen.push(b);
            if done {
                break;
            }
        }
        if seen.iter().any(|b| covers(b, &kept)) {
            break;
        }
    }
    running.store(false, Ordering::SeqCst);
    let _ = std::fs::write(temp.join("stop"), b"stop");
    let _ = handle.join();
    let _ = std::fs::remove_dir_all(&temp);
    assert!(seen.iter().any(|b| b.contains(&kept) || b.is_empty()), "the kept file is reported: {seen:?}");
    assert!(seen.iter().flatten().all(|p| !p.starts_with(temp.join("target"))), "nothing under target/ is queued: {seen:?}");
}

#[test]
fn a_file_written_without_a_pause_is_sent_once_per_gap_and_the_last_write_wins() {
    let temp = std::env::temp_dir().join(format!("watch_gap_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let temp = temp.canonicalize().unwrap();
    let busy = temp.join("busy.log");
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let t = temp.clone();
    let handle = std::thread::spawn(move || {
        // SAFETY of the test: the variable is read once, when the loop starts
        std::env::set_var("TRACEWISE_MIN_GAP_MS", "1000");
        let _ = watch_loop(
            &t,
            50,
            r,
            move |paths| {
                let content = paths.iter().find(|p| p.ends_with("busy.log")).and_then(|p| std::fs::read_to_string(p).ok());
                let _ = tx.send((Instant::now(), content, paths.is_empty()));
                true
            },
            || {},
        );
    });
    // readiness: write a probe file until the watcher reports it, so the timed part starts with a listening watcher
    let probe = temp.join("ready");
    for i in 0..20 {
        std::fs::write(&probe, format!("{i}")).unwrap();
        if rx.recv_timeout(Duration::from_millis(500)).is_ok() {
            break;
        }
    }
    std::thread::sleep(Duration::from_millis(1200)); // the gap after the probe's own send
    while rx.try_recv().is_ok() {}
    let start = Instant::now();
    let mut n = 0;
    while start.elapsed() < Duration::from_secs(3) {
        n += 1;
        std::fs::write(&busy, format!("write {n}")).unwrap();
        std::thread::sleep(Duration::from_millis(30));
    }
    let last = format!("write {n}");
    let wrote_until = Instant::now();
    let mut batches = Vec::new();
    // the same final content again (events can be dropped on a busy machine): any event must then send the last write
    for _ in 0..3 {
        while let Ok(b) = rx.recv_timeout(Duration::from_millis(1500)) {
            batches.push(b);
        }
        std::fs::write(&busy, &last).unwrap();
    }
    while let Ok(b) = rx.recv_timeout(Duration::from_millis(2500)) {
        batches.push(b);
    }
    running.store(false, Ordering::SeqCst);
    let _ = std::fs::write(temp.join("stop"), b"stop");
    let _ = handle.join();
    let _ = std::fs::remove_dir_all(&temp);

    // 3 s of writes with a 1 s gap: sends at about 0, 1, 2 and 3 s (without the gap, one every 0.4 s, so about 7)
    let with_file = batches.iter().filter(|b| b.1.is_some() && b.0 <= wrote_until).count();
    let reconciles = batches.iter().filter(|b| b.2).count();
    assert!(reconciles > 0 || with_file <= 5, "100 writes in 3 s give at most about 4 sends in 3 s, got {with_file}");
    if reconciles == 0 {
        assert_eq!(batches.iter().rev().find_map(|b| b.1.clone()).as_deref(), Some(last.as_str()), "the last write is the last one sent");
    }
}
