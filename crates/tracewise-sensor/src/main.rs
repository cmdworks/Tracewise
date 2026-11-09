//! `tracewise-sensor`: runs Apple's `eslogger` on behalf of one user and forwards the file events under that user's
//! repositories to the user's daemon (`tracewise-watchd`). It is meant to run as root from a LaunchDaemon, but it
//! keeps nothing and listens to nobody: it connects to the user's socket and does what the daemon says.
//!
//!   tracewise-sensor --socket PATH          run (reconnects forever)
//!   tracewise-sensor --parse --prefix DIR   read eslogger lines on stdin, print the compact events (for tests)
//!
//! Environment: `TRACEWISE_ESLOGGER` replaces /usr/bin/eslogger (tests; then root is not required),
//! `TRACEWISE_SENSOR_BACKOFF_SECS` (30) pause after a failed start, `TRACEWISE_SENSOR_FORMAT_SECS` (600) pause after "format".
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracewise_sensor::*;

fn arg_values(args: &[String], flag: &str) -> Vec<String> {
    args.windows(2).filter(|w| w[0] == flag).map(|w| w[1].clone()).collect()
}

/// The running `eslogger`, so that stopping the sensor (SIGTERM from launchd, `tracewise strict uninstall`) stops it too: it
/// would otherwise go on alone, at several percent of a core, with nobody reading it.
static CHILD_PID: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

extern "C" fn terminate(_: i32) {
    let pid = CHILD_PID.load(Ordering::SeqCst);
    if pid > 1 {
        let _ = tracewise_platform::signal::terminate_process(pid);
    }
    std::process::exit(0);
}

fn main() {
    tracewise_platform::signal::setup_stop_signals(terminate);
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--parse") {
        std::process::exit(parse_mode(&args));
    }
    let Some(socket) = arg_values(&args, "--socket").pop() else {
        eprintln!("usage: tracewise-sensor --socket PATH | --parse --prefix DIR [--events light|full]");
        std::process::exit(2);
    };
    loop {
        if let Ok(stream) = tracewise_platform::socket::connect_unix(Path::new(&socket)) {
            serve(stream);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Offline: the same filter the live sensor uses. Prints one compact event per line, a JSON summary on stderr.
fn parse_mode(args: &[String]) -> i32 {
    let prefixes = arg_values(args, "--prefix");
    let preset = arg_values(args, "--events").pop().unwrap_or_else(|| "light".into());
    let needles: Vec<String> = prefixes.iter().flat_map(|p| prefix_needles(p)).collect();
    let wanted = events_for(&preset);
    let (mut seen, mut forwarded, mut skipped) = (0u64, 0u64, 0u64);
    let stdin = std::io::stdin();
    for line in stdin.lock().lines().map_while(Result::ok) {
        seen += 1;
        match process_line(&line, &needles, &prefixes, wanted) {
            Parsed::Event(ev) => {
                forwarded += 1;
                println!("{}", serde_json::to_string(&ev).unwrap_or_default());
            }
            Parsed::Skipped => skipped += 1,
            Parsed::Ignored => {}
        }
    }
    eprintln!("{}", serde_json::json!({"seen": seen, "forwarded": forwarded, "skipped": skipped}));
    0
}

#[derive(Default)]
struct Shared {
    /// the daemon closed the connection or rejected us: stop eslogger and reconnect
    gone: AtomicBool,
    active: AtomicBool,
    running: AtomicBool,
    seen: AtomicU64,
    forwarded: AtomicU64,
    skipped: AtomicU64,
    /// lines that mention a watched folder, and how many of those we could not read
    mentions: AtomicU64,
    unreadable: AtomicU64,
    reason: Mutex<Option<String>>,
    stderr_text: Mutex<String>,
}

fn env_secs(name: &str, default: u64) -> Duration {
    Duration::from_secs(std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default))
}

fn macos_version() -> Option<String> {
    let out = Command::new("sw_vers").arg("-productVersion").output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

fn send(stream: &Mutex<UnixStream>, msg: &FromSensor) -> bool {
    let Ok(mut line) = serde_json::to_string(msg) else {
        return true;
    };
    line.push('\n');
    stream.lock().map(|mut s| s.write_all(line.as_bytes()).is_ok()).unwrap_or(false)
}

fn status(sh: &Shared, macos: &Option<String>) -> FromSensor {
    FromSensor::Status {
        st: Status {
            running: sh.running.load(Ordering::SeqCst),
            seen: sh.seen.load(Ordering::SeqCst),
            forwarded: sh.forwarded.load(Ordering::SeqCst),
            skipped: sh.skipped.load(Ordering::SeqCst),
            reason: sh.reason.lock().ok().and_then(|r| r.clone()),
            macos: macos.clone(),
        },
    }
}

fn stop_child(child: &mut Option<Child>) {
    if let Some(mut c) = child.take() {
        let _ = c.kill();
        let _ = c.wait();
        CHILD_PID.store(0, Ordering::SeqCst);
    }
}

fn check_running_child(child: &mut Option<Child>, sh: &Shared, started: Instant, pause_until: &mut Instant, last_status: &mut Instant) {
    let Some(c) = child.as_mut() else {
        return;
    };
    let want = sh.active.load(Ordering::SeqCst);
    if !want {
        stop_child(child);
        sh.running.store(false, Ordering::SeqCst);
        *last_status -= Duration::from_secs(10);
    } else if let Ok(Some(_exit)) = c.try_wait() {
        std::thread::sleep(Duration::from_millis(150)); // let the stderr reader finish
        let text = sh.stderr_text.lock().map(|t| t.clone()).unwrap_or_default();
        let reason = if text.contains("Full Disk Access") || text.contains("NOT_PERMITTED") {
            "full-disk-access".to_string()
        } else {
            format!("exited: {}", text.lines().next().unwrap_or("no message"))
        };
        *sh.reason.lock().unwrap() = Some(reason);
        stop_child(child);
        sh.running.store(false, Ordering::SeqCst);
        *pause_until = Instant::now() + env_secs("TRACEWISE_SENSOR_BACKOFF_SECS", 30);
        *last_status -= Duration::from_secs(10);
    } else {
        // the format watchdog: lines that mention our folders but none of which we can read
        let m = sh.mentions.load(Ordering::SeqCst);
        if m >= 20 && sh.unreadable.load(Ordering::SeqCst) == m && started.elapsed() > Duration::from_millis(500) {
            stop_child(child);
            sh.running.store(false, Ordering::SeqCst);
            *sh.reason.lock().unwrap() = Some("format".to_string());
            *pause_until = Instant::now() + env_secs("TRACEWISE_SENSOR_FORMAT_SECS", 600);
            *last_status -= Duration::from_secs(10);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_and_attach(
    sh: &Arc<Shared>,
    out: &Arc<Mutex<UnixStream>>,
    needles: &Arc<Vec<String>>,
    prefixes: &Arc<Vec<String>>,
    wanted: &'static [&'static str],
    pause_until: &mut Instant,
    last_status: &mut Instant,
) -> Option<Child> {
    match spawn_eslogger(wanted) {
        Ok(mut c) => {
            CHILD_PID.store(c.id() as i32, Ordering::SeqCst);
            sh.mentions.store(0, Ordering::SeqCst);
            sh.unreadable.store(0, Ordering::SeqCst);
            *sh.reason.lock().unwrap() = None;
            sh.stderr_text.lock().unwrap().clear();
            if let Some(stdout) = c.stdout.take() {
                let (sh, out, needles, prefixes) = (sh.clone(), out.clone(), needles.clone(), prefixes.clone());
                std::thread::spawn(move || {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        sh.seen.fetch_add(1, Ordering::Relaxed);
                        let mentioned = mentions_prefix(&line, &needles);
                        if mentioned {
                            sh.mentions.fetch_add(1, Ordering::Relaxed);
                        }
                        match process_line(&line, &needles, &prefixes, wanted) {
                            Parsed::Event(ev) => {
                                sh.forwarded.fetch_add(1, Ordering::Relaxed);
                                if !send(&out, &FromSensor::Event(ev)) {
                                    return;
                                }
                            }
                            Parsed::Skipped => {
                                sh.skipped.fetch_add(1, Ordering::Relaxed);
                                if mentioned {
                                    sh.unreadable.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Parsed::Ignored => {}
                        }
                    }
                });
            }
            if let Some(stderr) = c.stderr.take() {
                let sh = sh.clone();
                std::thread::spawn(move || {
                    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                        if let Ok(mut t) = sh.stderr_text.lock() {
                            if t.len() < 2000 {
                                t.push_str(&line);
                                t.push('\n');
                            }
                        }
                    }
                });
            }
            sh.running.store(true, Ordering::SeqCst);
            *last_status -= Duration::from_secs(10);
            Some(c)
        }
        Err(reason) => {
            *sh.reason.lock().unwrap() = Some(reason);
            *pause_until = Instant::now() + env_secs("TRACEWISE_SENSOR_BACKOFF_SECS", 30);
            *last_status -= Duration::from_secs(10);
            None
        }
    }
}

fn serve(stream: UnixStream) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    let Ok(ToSensor::Hello { prefixes, events, .. }) = serde_json::from_str::<ToSensor>(first.trim()) else {
        return; // rejected, or not a daemon
    };
    let needles: Arc<Vec<String>> = Arc::new(prefixes.iter().flat_map(|p| prefix_needles(p)).collect());
    let prefixes = Arc::new(prefixes);
    let wanted = events_for(&events);
    let out = Arc::new(Mutex::new(stream));
    let sh = Arc::new(Shared::default());
    let macos = macos_version();

    // commands from the daemon
    {
        let sh = sh.clone();
        std::thread::spawn(move || {
            for line in reader.lines().map_while(Result::ok) {
                match serde_json::from_str::<ToSensor>(&line) {
                    Ok(ToSensor::Start) => sh.active.store(true, Ordering::SeqCst),
                    Ok(ToSensor::Stop) => sh.active.store(false, Ordering::SeqCst),
                    Ok(ToSensor::Reject) => break,
                    _ => {}
                }
            }
            sh.gone.store(true, Ordering::SeqCst);
        });
    }

    let mut child: Option<Child> = None;
    let mut started = Instant::now();
    let mut pause_until = Instant::now();
    let mut last_status = Instant::now() - Duration::from_secs(10);
    loop {
        std::thread::sleep(Duration::from_millis(100));
        if sh.gone.load(Ordering::SeqCst) {
            stop_child(&mut child);
            return;
        }

        if child.is_some() {
            check_running_child(&mut child, &sh, started, &mut pause_until, &mut last_status);
        } else if sh.active.load(Ordering::SeqCst) && Instant::now() >= pause_until {
            if let Some(c) = spawn_and_attach(&sh, &out, &needles, &prefixes, wanted, &mut pause_until, &mut last_status) {
                child = Some(c);
                started = Instant::now();
            }
        }

        if last_status.elapsed() >= Duration::from_secs(2) {
            last_status = Instant::now();
            if !send(&out, &status(&sh, &macos)) {
                stop_child(&mut child);
                return;
            }
        }
    }
}

fn spawn_eslogger(events: &[&str]) -> Result<Child, String> {
    let (program, needs_root) = match std::env::var("TRACEWISE_ESLOGGER") {
        Ok(p) if !p.is_empty() => (p, false),
        _ => ("/usr/bin/eslogger".to_string(), true),
    };
    if needs_root && tracewise_platform::socket::current_uid() != 0 {
        return Err("not-root".into());
    }
    if !Path::new(&program).exists() {
        return Err("missing".into());
    }
    Command::new(&program)
        .args(events)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("exited: cannot start {program}: {e}"))
}
