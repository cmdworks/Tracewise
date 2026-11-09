//! The daemon's side of the strict sensor: a socket that only root may talk to, on-demand start and stop of the
//! sensor, and the answer to "which agent (if any) wrote this file".
//!
//! Trust: the socket is 0600 and a peer is accepted only when it is root. (`TRAIL_SENSOR_ALLOW_USER=1`, read from
//! the daemon's own environment, lets the same user connect: for tests with a fake eslogger. An agent cannot set the
//! daemon's environment.)
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use tracewise_proc::{agent_for_writer, process_chain, AgentTable};
use tracewise_sensor::{FromSensor, SensorEvent, Status, ToSensor};

const HINT_TTL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct Hint {
    /// the agent that wrote it; `None` means no agent was involved (a person)
    pub agent: Option<String>,
    pub kind: String,
    at: Instant,
}

#[derive(Default)]
struct Counters {
    connected: bool,
    rejected: u64,
    events: u64,
    last_event: Option<String>,
    sensor: Status,
    active: bool,
}

pub struct Server {
    /// (work tree, data folder) of every repository the daemon watches
    repos: Mutex<Vec<(PathBuf, PathBuf)>>,
    trail: PathBuf,
    home: PathBuf,
    hints: Mutex<HashMap<String, Hint>>,
    counters: Mutex<Counters>,
    conn: Mutex<Option<UnixStream>>,
    /// signalled when a sensor connects: the control thread sleeps until then (no sensor, no wake-ups)
    connected: Condvar,
    table: AgentTable,
    want: AtomicBool,
}

/// `/private/var/x` and `/var/x` are the same file; hints and file-system events must meet on one spelling.
fn canon(p: &str) -> String {
    tracewise_platform::paths::normalize_virtual_path(Path::new(p)).to_string_lossy().to_string()
}

/// The folder under both of its names: macOS reports `/private/var/...`, a program may say `/var/...`.
fn spellings(p: &str) -> Vec<String> {
    tracewise_platform::paths::path_spellings(p)
}

fn now_iso() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{secs}")
}

fn peer_uid(stream: &UnixStream) -> Option<u32> {
    tracewise_platform::socket::peer_uid(stream).ok()
}

fn send(stream: &mut UnixStream, msg: &ToSensor) -> bool {
    let Ok(mut line) = serde_json::to_string(msg) else {
        return false;
    };
    line.push('\n');
    stream.write_all(line.as_bytes()).is_ok()
}

impl Server {
    pub fn start(trail: &Path) -> Option<Arc<Server>> {
        if std::env::var("TRAIL_SENSOR_DISABLE").as_deref() == Ok("1") {
            return None;
        }
        let home = std::env::var("TRAIL_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".trail")))
            .ok()?;
        let _ = std::fs::create_dir_all(&home);
        let sock = home.join("sensor.sock");
        let listener = tracewise_platform::socket::listen_restricted(&sock, 0o600).ok()?;
        let srv = Arc::new(Server {
            repos: Mutex::new(Vec::new()),
            trail: trail.to_path_buf(),
            home,
            hints: Mutex::new(HashMap::new()),
            counters: Mutex::new(Counters::default()),
            conn: Mutex::new(None),
            connected: Condvar::new(),
            table: AgentTable::load_default(),
            want: AtomicBool::new(false),
        });
        srv.write_status();
        {
            let srv = srv.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let srv = srv.clone();
                    std::thread::spawn(move || srv.handle_peer(stream));
                }
            });
        }
        {
            let srv = srv.clone();
            std::thread::spawn(move || srv.control_loop());
        }
        Some(srv)
    }

    fn handle_peer(self: Arc<Self>, mut stream: UnixStream) {
        let me = tracewise_platform::socket::current_uid();
        let ok = match peer_uid(&stream) {
            Some(0) => true,
            Some(uid) => uid == me && std::env::var("TRAIL_SENSOR_ALLOW_USER").as_deref() == Ok("1"),
            None => false,
        };
        if !ok {
            let _ = send(&mut stream, &ToSensor::Reject);
            if let Ok(mut c) = self.counters.lock() {
                c.rejected += 1;
                c.sensor.reason = Some("rejected-peer".into());
            }
            self.write_status();
            return;
        }
        let events = std::env::var("TRAIL_SENSOR_EVENTS")
            .ok()
            .or_else(|| {
                // `trail strict install --events full` leaves its choice here
                let home = std::env::var("TRAIL_HOME")
                    .map(std::path::PathBuf::from)
                    .ok()
                    .or_else(|| std::env::var("HOME").ok().map(|h| std::path::Path::new(&h).join(".trail")))?;
                std::fs::read_to_string(home.join("sensor-events")).ok().map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "light".into());
        let hello = ToSensor::Hello {
            v: 1,
            prefixes: self.repos.lock().unwrap().iter().flat_map(|(wt, _)| spellings(&wt.to_string_lossy())).collect(),
            events,
        };
        let Ok(mut writer) = stream.try_clone() else {
            return;
        };
        if !send(&mut writer, &hello) {
            return;
        }
        let first = if self.want.load(Ordering::SeqCst) { ToSensor::Start } else { ToSensor::Stop };
        let _ = send(&mut writer, &first);
        *self.conn.lock().unwrap() = Some(writer);
        self.connected.notify_all();
        self.counters.lock().unwrap().connected = true;
        self.write_status();

        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            match serde_json::from_str::<FromSensor>(&line) {
                Ok(FromSensor::Event(ev)) => self.handle_event(ev),
                Ok(FromSensor::Status { st }) => {
                    self.counters.lock().unwrap().sensor = st;
                    self.write_status();
                }
                Err(_) => {}
            }
        }
        *self.conn.lock().unwrap() = None;
        self.counters.lock().unwrap().connected = false;
        self.write_status();
    }

    fn handle_event(&self, ev: SensorEvent) {
        let w = &ev.writer;
        let mut chain = Vec::new();
        // the writer is often gone already (sed): its parent shell is not. A job reparented to launchd keeps `original_ppid`.
        for parent in [w.ppid, w.original_ppid].into_iter().flatten() {
            if parent > 1 {
                chain = process_chain(parent as u32);
                if !chain.is_empty() {
                    break;
                }
            }
        }
        let agent = agent_for_writer(&self.table, &w.exe, w.signing_id.as_deref(), w.pid as u32, &chain);
        // A shell or interpreter whose parent is gone (a `nohup` job, a cron script) has no one we can name: that is not
        // "a person". Leaving it unhinted lets the windows (an agent's background window) decide.
        let orphan_script = agent.is_none()
            && chain.is_empty()
            && std::path::Path::new(&w.exe).file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                ["sh", "bash", "zsh", "dash", "ksh", "fish", "python", "python3", "node", "ruby", "perl", "php"].contains(&n)
            });
        let mut tamper = Vec::new();
        if !orphan_script {
            let mut hints = self.hints.lock().unwrap();
            hints.retain(|_, h| h.at.elapsed() < HINT_TTL);
            for p in &ev.paths {
                let key = canon(p);
                hints.insert(key.clone(), Hint { agent: agent.clone(), kind: ev.kind.clone(), at: Instant::now() });
                if matches!(ev.kind.as_str(), "setflags" | "setextattr" | "deleteextattr") {
                    tamper.push(key);
                }
            }
        }
        {
            let mut c = self.counters.lock().unwrap();
            c.events += 1;
            c.last_event = Some(now_iso());
        }
        for path in tamper {
            let line = serde_json::json!({"kind": ev.kind, "path": path, "agent": agent, "hinted": true}).to_string();
            let Some(wt) = self.repo_for(&path) else {
                continue;
            };
            let trail = self.trail.clone();
            std::thread::spawn(move || run_scan_paths(&trail, &wt, &line));
        }
    }

    /// The hint for a path that just changed, if the sensor saw who wrote it in the last few seconds.
    pub fn take_hint(&self, path: &Path) -> Option<Hint> {
        let key = canon(&path.to_string_lossy());
        let mut hints = self.hints.lock().ok()?;
        let hint = hints.get(&key).cloned()?;
        (hint.at.elapsed() < HINT_TTL).then(|| {
            hints.remove(&key);
            hint
        })
    }

    /// Start the sensor while an agent works (or a background job is alive), stop it a while after.
    fn control_loop(self: Arc<Self>) {
        let hold = Duration::from_secs(std::env::var("TRAIL_SENSOR_HOLD_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(120));
        let always = std::env::var("TRAIL_SENSOR_ALWAYS").as_deref() == Ok("1");
        let mut last_true: Option<Instant> = None;
        loop {
            {
                // no sensor connected: nothing to switch, so sleep until one connects
                let mut conn = self.conn.lock().unwrap();
                while conn.is_none() {
                    conn = self.connected.wait(conn).unwrap();
                }
            }
            std::thread::sleep(Duration::from_secs(std::env::var("TRAIL_SENSOR_POLL_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(2)));
            if self.conn.lock().unwrap().is_none() {
                continue; // no sensor connected: nothing to switch, and no polling
            }
            let busy = always || self.agent_active();
            if busy {
                last_true = Some(Instant::now());
            }
            let want = busy || last_true.is_some_and(|t| t.elapsed() < hold);
            let changed = self.want.swap(want, Ordering::SeqCst) != want;
            if changed {
                if let Some(c) = self.conn.lock().unwrap().as_mut() {
                    let _ = send(c, if want { &ToSensor::Start } else { &ToSensor::Stop });
                }
            }
            self.counters.lock().unwrap().active = want;
            self.write_status();
        }
    }

    fn agent_active(&self) -> bool {
        let repos: Vec<PathBuf> = self.repos.lock().unwrap().iter().map(|(wt, _)| wt.clone()).collect();
        repos.iter().any(|wt| {
            Command::new(&self.trail)
                .arg("-C")
                .arg(wt)
                .args(["daemon", "active"])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
                .unwrap_or(false)
        })
    }

    /// The watched repository a (canonical) path lies in.
    fn repo_for(&self, path: &str) -> Option<PathBuf> {
        let path = canon(path);
        self.repos
            .lock()
            .unwrap()
            .iter()
            .find(|(wt, _)| Path::new(&path).starts_with(canon(&wt.to_string_lossy())))
            .map(|(wt, _)| wt.clone())
    }

    /// The repositories the daemon watches changed: a connected sensor must hear the new folders, so it is
    /// disconnected and comes back with a fresh hello.
    pub fn set_repos(&self, repos: Vec<(PathBuf, PathBuf)>) {
        {
            let mut current = self.repos.lock().unwrap();
            if *current == repos {
                return;
            }
            *current = repos;
        }
        if let Some(c) = self.conn.lock().unwrap().take() {
            let _ = c.shutdown(std::net::Shutdown::Both);
        }
    }

    fn write_status(&self) {
        let c = self.counters.lock().unwrap();
        let doc = serde_json::json!({
            "connected": c.connected,
            "active": c.active,
            "running": c.sensor.running,
            "reason": c.sensor.reason,
            "macos": c.sensor.macos,
            "seen": c.sensor.seen,
            "forwarded": c.sensor.forwarded,
            "skipped": c.sensor.skipped,
            "events": c.events,
            "last_event": c.last_event,
            "rejected": c.rejected,
            "updated": now_iso(),
            "work_trees": self
                .repos
                .lock()
                .unwrap()
                .iter()
                .map(|(wt, _)| wt.clone())
                .collect::<Vec<_>>(),
        });
        let tmp = self.home.join("sensor.status.json.tmp");
        if std::fs::write(&tmp, doc.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, self.home.join("sensor.status.json"));
        }
    }
}

/// `trail -C <work tree> daemon scan-paths`, one JSON line per path on stdin. True when it exited 0.
pub fn run_scan_paths(trail: &Path, work_tree: &Path, lines: &str) -> bool {
    let mut cmd = Command::new(trail);
    cmd.arg("-C")
        .arg(work_tree)
        .args(["daemon", "scan-paths"])
        .env("TRAIL_GIT_DEPTH", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    let Ok(mut child) = cmd.spawn() else { return false };
    if let Some(mut pipe) = child.stdin.take() {
        let _ = pipe.write_all(lines.as_bytes());
        let _ = pipe.write_all(b"\n");
    }
    child.wait().is_ok_and(|s| s.success())
}
