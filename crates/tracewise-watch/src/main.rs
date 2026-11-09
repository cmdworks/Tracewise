//! `tracewise-watchd`: the file watcher behind `tracewise daemon run`.
//!
//! It is a separate program so that `tracewise` itself does not link CoreServices/CoreFoundation: hooks and shell
//! commands start `tracewise` many times a second, and loading those frameworks costs about a millisecond each time.
//! The watcher only notices changes; recording them is done by `tracewise daemon scan-paths`, `tracewise daemon sweep`
//! and `tracewise scan`, which this program starts from the `tracewise` binary next to it.
mod server;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

fn tracewise_home() -> PathBuf {
    if let Ok(home) = std::env::var("TRACEWISE_HOME") {
        PathBuf::from(home)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".tracewise")
    } else {
        PathBuf::from(".tracewise")
    }
}

extern "C" fn stop(_: i32) {
    let _ = std::fs::remove_file(tracewise_home().join("daemon.pid"));
    std::process::exit(0);
}

/// `tracewise` next to this program (never searched on PATH: that could find a wrapper or another install).
fn sibling_tracewise() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?.canonicalize().ok()?;
    let tracewise = me.parent()?.join("tracewise");
    tracewise.is_file().then_some(tracewise)
}

/// Size and modification time of the database and its write-ahead log: changes when anything was recorded.
type DbStamp = Vec<(u64, std::time::SystemTime)>;

fn db_stamp(data_dir: &Path) -> DbStamp {
    ["tracewise.db", "tracewise.db-wal"]
        .iter()
        .filter_map(|f| std::fs::metadata(data_dir.join(f)).ok())
        .map(|m| (m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)))
        .collect()
}

/// True when `tracewise` ran and exited 0.
fn run(tracewise: &Path, work_tree: &Path, args: &[&str], stdin: Option<Vec<u8>>) -> bool {
    let mut cmd = Command::new(tracewise);
    cmd.arg("-C").arg(work_tree).args(args).env("TRACEWISE_GIT_DEPTH", "1").stdout(Stdio::null()).stderr(Stdio::inherit());
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    let Ok(mut child) = cmd.spawn() else { return false };
    if let (Some(data), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let _ = pipe.write_all(&data);
    }
    child.wait().is_ok_and(|s| s.success())
}

/// Write end of the pipe a SIGHUP wakes the main thread through (a signal handler may only do async-safe things).
static WAKE_FD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

extern "C" fn hup(_: i32) {
    let fd = WAKE_FD.load(std::sync::atomic::Ordering::Relaxed);
    tracewise_platform::signal::WakePipe::wake_async_signal_safe(fd);
}

/// Watches one repository for ever: a catch-up scan, then file events, plus the lock sweep.
fn watch_repo(tracewise: PathBuf, work_tree: PathBuf, data_dir: PathBuf, sensor: Option<Arc<server::Server>>, debounce_ms: u64) {
    // Catch up on anything written while the daemon was not running. It runs just after the watcher is
    // listening, so a write cannot fall between the two; a file seen by both is recorded once.
    {
        let (tracewise, wt) = (tracewise.clone(), work_tree.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            run(&tracewise, &wt, &["scan"], None);
            // git apps that never run the wrapper are covered by git hooks; a repository that cannot have them (core.hooksPath)
            // just answers with an error, which is not a problem
            if std::env::var("TRACEWISE_NO_GIT_HOOKS").as_deref() != Ok("1") {
                run(&tracewise, &wt, &["git-hooks", "install"], None);
            }
        });
    }

    let (t1, w1) = (tracewise.clone(), work_tree.clone());
    let w2 = work_tree.clone();
    // An ignored path is dropped before it is queued, unless it was recorded before (its deletion or edit still counts).
    let skip = {
        let (wt, db) = (work_tree.clone(), tracewise_core::Database::open_hot(&data_dir.join("tracewise.db")).ok());
        let mut rules = tracewise_core::ignore_rules::IgnoreRules::load(&wt);
        move |p: &Path| {
            let Ok(rel) = p.strip_prefix(&wt) else { return false };
            // the rules are cached: an edited rules file is queued and starts them afresh
            if matches!(rel.file_name().and_then(|n| n.to_str()), Some(".gitignore" | ".tracewiseignore")) {
                rules = tracewise_core::ignore_rules::IgnoreRules::load(&wt);
                return false;
            }
            let known = db.as_ref().is_some_and(|d| d.get_file(&rel.to_string_lossy()).ok().flatten().is_some());
            !known && rules.is_ignored(rel)
        }
    };
    let _ = tracewise_watch::watch_loop_filtered(
        &work_tree,
        debounce_ms,
        Arc::new(AtomicBool::new(true)),
        skip,
        move |changed| {
            // an empty list: the watcher reported an error, so events may have been lost; look at the whole tree
            if changed.is_empty() {
                return run(&t1, &w1, &["scan"], None);
            }
            // one JSON line per path; `agent` only when the sensor saw the writer
            let mut lines = String::new();
            for p in changed {
                let line = match sensor.as_ref().and_then(|s| s.take_hint(p)) {
                    Some(h) => {
                        serde_json::json!({"path": p, "hinted": true, "agent": h.agent, "kind": h.kind})
                    }
                    None => serde_json::json!({"path": p}),
                };
                lines.push_str(&line.to_string());
                lines.push('\n');
            }
            server::run_scan_paths(&t1, &w1, lines.trim_end())
        },
        {
            // In this process, over one open connection: a sweep every 30 s must not start anything.
            // opened now, not at the first sweep: SQLite's one-time setup costs a burst of wake-ups
            let db = tracewise_core::Database::open_hot(&data_dir.join("tracewise.db")).ok();
            // The database stamp at the last sweep that found no lock held: while it is unchanged nothing can have
            // been locked or have expired, so the tick only looks at two files and never reads the database
            // (a read that has to wait for a busy disk is a wake-up).
            let mut quiet: Option<DbStamp> = None;
            // the first sweep now, so the first tick (30 s in) is already a quiet one
            if let Some(db) = &db {
                let _ = db.clean_expired_locks(&w2);
                let none_held = db.get_active_locks().map(|l| l.is_empty()).unwrap_or(false);
                quiet = none_held.then(|| db_stamp(&data_dir));
            }
            move || {
                if quiet.as_ref().is_some_and(|q| *q == db_stamp(&data_dir)) {
                    return;
                }
                if let Some(db) = &db {
                    let _ = db.clean_expired_locks(&w2);
                    let none_held = db.get_active_locks().map(|l| l.is_empty()).unwrap_or(false);
                    quiet = none_held.then(|| db_stamp(&data_dir));
                }
            }
        },
    );
}

/// One daemon for every repository in `$TRACEWISE_HOME/repos`. A repository added later (`tracewise init`, `tracewise daemon
/// start`) sends SIGHUP and is picked up at once; with nothing to do this thread blocks in poll(2), no wake-ups.
fn main() {
    let Some(tracewise) = sibling_tracewise() else {
        eprintln!("tracewise-watchd: the `tracewise` binary must be in the same folder");
        std::process::exit(1);
    };
    let wake_pipe = tracewise_platform::signal::WakePipe::new().expect("wake pipe");
    WAKE_FD.store(wake_pipe.write_fd(), std::sync::atomic::Ordering::Relaxed);
    tracewise_platform::signal::setup_daemon_signals(stop, hup);

    let debounce_ms = std::env::var("TRACEWISE_DEBOUNCE_MS").ok().and_then(|s| s.parse::<u64>().ok()).unwrap_or(300);

    // The strict sensor (opt-in, root, on demand): who wrote a file, when it can tell.
    let sensor = server::Server::start(&tracewise);
    let mut watched: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    loop {
        let repos = tracewise_core::repos::list();
        for r in &repos {
            if watched.insert(r.work_tree.clone()) {
                let (t, wt, dd, s) = (tracewise.clone(), r.work_tree.clone(), r.data_dir.clone(), sensor.clone());
                std::thread::spawn(move || watch_repo(t, wt, dd, s, debounce_ms));
            }
        }
        if let Some(s) = &sensor {
            s.set_repos(repos.iter().map(|r| (r.work_tree.clone(), r.data_dir.clone())).collect());
        }
        // sleep until SIGHUP (or any byte on the pipe)
        wake_pipe.wait_for_wake();
    }
}
