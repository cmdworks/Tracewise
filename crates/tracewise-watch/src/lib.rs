//! File system monitoring and debounce handling for Tracewise.

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Most changed paths held at once before the watcher switches to one whole-tree scan.
const MAX_PENDING_PATHS: usize = 10_000;

/// Runs the file system watcher on `work_tree` until `running` becomes false.
///
/// `on_change` gets the changed paths (each once) and returns whether they were recorded; `false` keeps them and repeats the
/// call after a growing pause (up to 30 s). An **empty** slice means "events may be lost, look at the whole tree" (same retry).
/// `on_sweep` is called periodically (every 30 s by default).
pub fn watch_loop<F, S>(
    work_tree: &Path,
    debounce_ms: u64,
    running: Arc<AtomicBool>,
    on_change: F,
    on_sweep: S,
) -> Result<(), notify::Error>
where
    F: FnMut(&[PathBuf]) -> bool,
    S: FnMut(),
{
    watch_loop_filtered(work_tree, debounce_ms, running, |_| false, on_change, on_sweep)
}

/// Like [`watch_loop`], but a path for which `skip` returns true never enters the queue: an ignored folder (`target/`,
/// `node_modules/`) then costs one check per event, not a pending path and a scan. `skip` gets the absolute path.
pub fn watch_loop_filtered<K, F, S>(
    work_tree: &Path,
    debounce_ms: u64,
    running: Arc<AtomicBool>,
    skip: K,
    on_change: F,
    on_sweep: S,
) -> Result<(), notify::Error>
where
    K: FnMut(&Path) -> bool,
    F: FnMut(&[PathBuf]) -> bool,
    S: FnMut(),
{
    watch_loop_capped(work_tree, debounce_ms, running, MAX_PENDING_PATHS, skip, on_change, on_sweep)
}

/// When each path was last sent, so a path written without a pause goes out once per `min_gap`.
struct SendGate {
    min_gap: Duration,
    last_flush: HashMap<PathBuf, Instant>,
}

impl SendGate {
    /// The pending paths that were not sent within the gap (older entries are forgotten after 60 s).
    fn ready(&mut self, pending: &HashSet<PathBuf>, now: Instant) -> Vec<PathBuf> {
        let keep = Duration::from_secs(60).max(self.min_gap);
        self.last_flush.retain(|_, t| now.duration_since(*t) < keep);
        let held = |p: &PathBuf| self.last_flush.get(p).is_some_and(|t| now.duration_since(*t) < self.min_gap);
        pending.iter().filter(|p| !held(p)).cloned().collect()
    }

    fn sent(&mut self, batch: &[PathBuf], pending: &mut HashSet<PathBuf>, now: Instant) {
        for p in batch {
            self.last_flush.insert(p.clone(), now);
            pending.remove(p);
        }
    }

    /// When the earliest held-back path may go out.
    fn next_due(&self, pending: &HashSet<PathBuf>) -> Option<Instant> {
        pending.iter().filter_map(|p| self.last_flush.get(p)).map(|t| *t + self.min_gap).min()
    }
}

fn watch_loop_capped<K, F, S>(
    work_tree: &Path,
    debounce_ms: u64,
    running: Arc<AtomicBool>,
    max_pending: usize,
    mut skip: K,
    mut on_change: F,
    mut on_sweep: S,
) -> Result<(), notify::Error>
where
    K: FnMut(&Path) -> bool,
    F: FnMut(&[PathBuf]) -> bool,
    S: FnMut(),
{
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(work_tree, RecursiveMode::Recursive)?;

    let debounce_duration = Duration::from_millis(debounce_ms);
    let max_batch_duration = Duration::from_millis(debounce_ms.saturating_mul(2).max(400));
    let mut pending_paths: HashSet<PathBuf> = HashSet::new();
    let mut need_reconcile = false;
    let mut first_event_time: Option<Instant> = None;
    let mut last_event_time: Option<Instant> = None;
    // after a failed call: no new attempt before this time
    let (mut retry_at, mut failures) = (None::<Instant>, 0u32);

    let sweep_secs = std::env::var("TRACEWISE_SWEEP_INTERVAL_SECS").ok().and_then(|s| s.parse::<u64>().ok()).unwrap_or(30);
    let sweep_interval = Duration::from_secs(sweep_secs.max(1));
    let mut last_sweep = Instant::now();
    // A path written without a pause is recorded at most once per `min_gap`; what arrives in between waits and goes out
    // with the latest content (the path stays pending until it is sent).
    let min_gap = Duration::from_millis(std::env::var("TRACEWISE_MIN_GAP_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(1000));
    let mut gate = SendGate { min_gap, last_flush: HashMap::new() };

    while running.load(Ordering::SeqCst) {
        let idle_timeout = Duration::from_secs(30).min(sweep_interval);
        let work = !pending_paths.is_empty() || need_reconcile;
        let timeout = match (work, last_event_time) {
            (true, Some(last)) => {
                let first = first_event_time.unwrap_or(last);
                let wait_debounce = debounce_duration.saturating_sub(last.elapsed());
                let wait_max = max_batch_duration.saturating_sub(first.elapsed());
                let wait_retry = retry_at.map(|t| t.saturating_duration_since(Instant::now())).unwrap_or_default();
                wait_debounce.min(wait_max).max(wait_retry).max(Duration::from_millis(1))
            }
            _ => idle_timeout,
        };

        match rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                // Tracewise's own database and git's files live under .git: they never start a debounce cycle.
                let before = pending_paths.len();
                for path in event.paths {
                    if !path.components().any(|c| c.as_os_str() == ".git") && !tracewise_platform::paths::is_os_junk(&path) && !skip(&path)
                    {
                        pending_paths.insert(path);
                    }
                }
                // a burst bigger than this is not tracked path by path: one whole-tree scan covers it (bounded memory)
                if pending_paths.len() > max_pending {
                    pending_paths.clear();
                    need_reconcile = true;
                }
                if pending_paths.len() > before || need_reconcile {
                    first_event_time.get_or_insert_with(Instant::now);
                    last_event_time = Some(Instant::now());
                }
            }
            Ok(Err(_)) => {
                // the watcher itself reported a problem: events may be lost, so a whole-tree look is due
                need_reconcile = true;
                first_event_time.get_or_insert_with(Instant::now);
                last_event_time = Some(Instant::now());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(notify::Error::generic("watcher channel disconnected"));
            }
        }

        let due = last_event_time.is_some_and(|last| {
            let first = first_event_time.unwrap_or(last);
            last.elapsed() >= debounce_duration || first.elapsed() >= max_batch_duration
        }) && retry_at.is_none_or(|t| Instant::now() >= t);
        if due && (!pending_paths.is_empty() || need_reconcile) {
            let now = Instant::now();
            let batch = if need_reconcile { Vec::new() } else { gate.ready(&pending_paths, now) };
            if batch.is_empty() && !need_reconcile {
                // everything pending was recorded a moment ago: wait for the earliest to be due again
                retry_at = gate.next_due(&pending_paths);
            } else if on_change(&batch) {
                gate.sent(&batch, &mut pending_paths, now);
                if need_reconcile {
                    pending_paths.clear(); // the whole-tree scan covered them
                }
                need_reconcile = false;
                failures = 0;
                if pending_paths.is_empty() {
                    first_event_time = None;
                    last_event_time = None;
                    retry_at = None;
                } else {
                    retry_at = gate.next_due(&pending_paths);
                }
            } else {
                failures += 1;
                let pause = debounce_duration.saturating_mul(1 << failures.min(10)).min(Duration::from_secs(30));
                retry_at = Some(Instant::now() + pause);
            }
        }

        if last_sweep.elapsed() >= sweep_interval {
            on_sweep();
            last_sweep = Instant::now();
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "test_lib.rs"]
mod tests;
