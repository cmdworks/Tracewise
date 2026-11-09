use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const ALLOWED_COMMANDS: &[&str] = &[
    "status",
    "log",
    "show",
    "file",
    "blame",
    "why",
    "diff",
    "undo",
    "redo",
    "restore",
    "pending",
    "note",
    "lock",
    "ps",
    "background",
    "commands",
    "verify",
    "gc",
    "doctor",
    "agent",
    "daemon",
    "intent",
    "config",
    "share",
];

const STRIPPED_ENV_VARS: &[&str] = &[
    "CLAUDECODE",
    "TRACEWISE_ACTOR",
    "TRACEWISE_WHY",
    "TRACEWISE_INTENT",
    "TRACEWISE_SESSION",
    "TRACEWISE_SHELL_ACTIVE",
    "CODEX_THREAD_ID",
    "CURSOR_TRACE_ID",
    "GEMINI_CLI",
];

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TracewiseResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub fn validate_tracewise_args(args: &[String]) -> Result<&str, String> {
    if args.is_empty() {
        return Err("no subcommand provided to tracewise".to_string());
    }
    let subcmd = args[0].as_str();
    if !ALLOWED_COMMANDS.contains(&subcmd) {
        return Err(format!("subcommand not permitted: '{subcmd}'"));
    }
    Ok(subcmd)
}

/// The work trees Tracewise knows (`<TRACEWISE_HOME>/repos`, one `work tree<TAB>data dir` line each), as real paths.
fn registered_repos(registry: &Path) -> Vec<PathBuf> {
    let text = std::fs::read_to_string(registry).unwrap_or_default();
    text.lines().filter_map(|l| std::fs::canonicalize(l.split('\t').next()?).ok()).collect()
}

fn registry_file() -> PathBuf {
    match std::env::var_os("TRACEWISE_HOME") {
        Some(h) => PathBuf::from(h),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".tracewise"),
    }
    .join("repos")
}

/// A repository the UI may act on: it must exist, resolve (symlinks included) to a registered work tree, and has no `..`.
pub fn validate_repo_path_in(repo: &Option<String>, registry: &Path) -> Result<Option<PathBuf>, String> {
    let Some(r) = repo else { return Ok(None) };
    if r.contains("..") {
        return Err(format!("traversal paths not permitted: '{r}'"));
    }
    let real = std::fs::canonicalize(r).map_err(|_| format!("repository path does not exist: '{r}'"))?;
    if !real.is_dir() {
        return Err(format!("repository path does not exist: '{r}'"));
    }
    if !registered_repos(registry).contains(&real) {
        return Err(format!("not a repository Tracewise knows: '{r}'"));
    }
    Ok(Some(real))
}

pub fn validate_repo_path(repo: &Option<String>) -> Result<Option<PathBuf>, String> {
    validate_repo_path_in(repo, &registry_file())
}

pub fn locate_tracewise_binary() -> Result<PathBuf, String> {
    if let Ok(override_bin) = std::env::var("TRACEWISE_BIN") {
        let p = PathBuf::from(override_bin);
        if p.is_file() {
            return Ok(p);
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let installed = PathBuf::from(home).join(".tracewise").join("bin").join("tracewise");
        if installed.is_file() {
            return Ok(installed);
        }
    }

    // Dev environment fallbacks: look up from current dir for target/debug/tracewise or target/release/tracewise
    let dev_candidates = [
        PathBuf::from("../../target/debug/tracewise"),
        PathBuf::from("../../target/release/tracewise"),
        PathBuf::from("../target/debug/tracewise"),
        PathBuf::from("target/debug/tracewise"),
    ];
    for cand in &dev_candidates {
        if cand.is_file() {
            return Ok(cand.clone());
        }
    }

    // PATH lookup
    if let Ok(output) = Command::new("which").arg("tracewise").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let p = PathBuf::from(path_str);
            if p.is_file() {
                return Ok(p);
            }
        }
    }

    Err("tracewise binary not found in ~/.tracewise/bin/tracewise, workspace target, or PATH".to_string())
}

pub fn build_tracewise_command(tracewise_bin: &Path, args: &[String], repo_path: Option<&Path>) -> Command {
    let mut cmd = Command::new(tracewise_bin);
    cmd.args(args);

    if let Some(repo) = repo_path {
        cmd.current_dir(repo);
    }

    // Strip agent environment variables so Tracewise acts as the person
    for var in STRIPPED_ENV_VARS {
        cmd.env_remove(var);
    }

    cmd
}

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

/// What a reader thread has collected so far; `done` is set at end of file.
#[derive(Default)]
struct Captured {
    bytes: Mutex<Vec<u8>>,
    done: AtomicBool,
}

/// Reads a pipe into `cap` up to `max` bytes; sets `over` when there is more, and stops (the caller then kills the process).
fn read_capped(mut pipe: impl Read, max: usize, over: &AtomicBool, cap: &Captured) {
    let mut chunk = [0u8; 8192];
    while let Ok(n) = pipe.read(&mut chunk) {
        if n == 0 {
            break;
        }
        let mut out = cap.bytes.lock().unwrap();
        if out.len() + n > max {
            let room = max - out.len();
            out.extend_from_slice(&chunk[..room]);
            over.store(true, Ordering::SeqCst);
            break;
        }
        out.extend_from_slice(&chunk[..n]);
    }
    cap.done.store(true, Ordering::SeqCst);
}

/// How long to wait for the pipes to close after the process has ended. A background process that inherited them
/// (a daemon the command started) keeps them open; the output collected so far is returned instead of waiting for it.
const PIPE_GRACE: Duration = Duration::from_millis(300);

/// Runs the command with a time limit and an output limit; a command over either is killed and reported as an error.
pub fn run_bounded(cmd: &mut Command, timeout: Duration, max_bytes: usize) -> Result<TracewiseResult, String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn tracewise process: {e}"))?;
    let over = Arc::new(AtomicBool::new(false));
    let (out_cap, err_cap) = (Arc::new(Captured::default()), Arc::new(Captured::default()));
    let (out_pipe, err_pipe) = (child.stdout.take().expect("piped"), child.stderr.take().expect("piped"));
    {
        let (o, c) = (over.clone(), out_cap.clone());
        std::thread::spawn(move || read_capped(out_pipe, max_bytes, &o, &c));
        let (o, c) = (over.clone(), err_cap.clone());
        std::thread::spawn(move || read_capped(err_pipe, max_bytes, &o, &c));
    }
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("failed to wait for tracewise: {e}"))? {
            break Ok(status);
        }
        if over.load(Ordering::SeqCst) {
            break Err(format!("tracewise printed more than {} MB; stopped", max_bytes / 1024 / 1024));
        }
        if start.elapsed() > timeout {
            break Err(format!("tracewise did not finish within {} seconds; stopped", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if status.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let grace = Instant::now();
    while !(out_cap.done.load(Ordering::SeqCst) && err_cap.done.load(Ordering::SeqCst)) && grace.elapsed() < PIPE_GRACE {
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = status?;
    // the reader closes the pipe when full, so the process may have ended by itself before the loop saw the flag
    if over.load(Ordering::SeqCst) {
        return Err(format!("tracewise printed more than {} MB; stopped", max_bytes / 1024 / 1024));
    }
    let take = |c: &Captured| String::from_utf8_lossy(&c.bytes.lock().unwrap()).into_owned();
    let mut stderr = take(&err_cap);
    if !(out_cap.done.load(Ordering::SeqCst) && err_cap.done.load(Ordering::SeqCst)) {
        stderr.push_str("\n(output may be incomplete: a background process kept the output open)");
    }
    Ok(TracewiseResult { stdout: take(&out_cap), stderr, exit_code: status.code().unwrap_or(-1) })
}

#[tauri::command]
async fn tracewise(args: Vec<String>, repo: Option<String>) -> Result<TracewiseResult, String> {
    validate_tracewise_args(&args)?;
    let validated_repo = validate_repo_path(&repo)?;
    let tracewise_bin = locate_tracewise_binary()?;

    let mut cmd = build_tracewise_command(&tracewise_bin, &args, validated_repo.as_deref());

    // runs on a blocking thread so a slow command cannot stall the app's async runtime
    tauri::async_runtime::spawn_blocking(move || run_bounded(&mut cmd, COMMAND_TIMEOUT, MAX_OUTPUT_BYTES))
        .await
        .map_err(|e| format!("tracewise task failed: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![tracewise])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{build_tracewise_command, validate_repo_path, validate_repo_path_in, validate_tracewise_args, ALLOWED_COMMANDS};
    use std::path::PathBuf;

    #[test]
    fn test_allow_list_permits_allowed_commands() {
        for cmd in ALLOWED_COMMANDS {
            let single = vec![cmd.to_string()];
            assert!(validate_tracewise_args(&single).is_ok());

            let with_json = vec![cmd.to_string(), "--json".to_string()];
            assert!(validate_tracewise_args(&with_json).is_ok());
        }
    }

    #[test]
    fn test_allow_list_refuses_disallowed_commands() {
        let disallowed = ["setup", "uninstall", "init", "push", "fetch", "rm", "cat", "bash"];
        for cmd in disallowed {
            let bad_args = vec![cmd.to_string()];
            let res = validate_tracewise_args(&bad_args);
            assert!(res.is_err(), "expected '{cmd}' to be rejected");
            assert!(res.unwrap_err().contains("not permitted"));
        }
        let empty: Vec<String> = Vec::new();
        assert!(validate_tracewise_args(&empty).is_err());
    }

    #[test]
    fn test_repo_path_validation_rejects_traversals() {
        let bad = Some("../foo/bar".to_string());
        assert!(validate_repo_path(&bad).is_err());

        let nonexistent = Some("/tmp/tracewise_nonexistent_path_test_12345".to_string());
        assert!(validate_repo_path(&nonexistent).is_err());

        assert!(validate_repo_path(&None).is_ok());
    }

    #[test]
    fn only_registered_repositories_are_accepted_even_through_a_symlink() {
        let tmp = std::env::temp_dir().join(format!("tracewise-ui-reg-{}", std::process::id()));
        let (known, other) = (tmp.join("known"), tmp.join("other"));
        std::fs::create_dir_all(&known).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let registry = tmp.join("repos");
        std::fs::write(&registry, format!("{}\t{}/.git/tracewise\n", known.display(), known.display())).unwrap();
        let some = |p: &std::path::Path| Some(p.to_string_lossy().to_string());
        let real = std::fs::canonicalize(&known).unwrap();
        assert_eq!(validate_repo_path_in(&some(&known), &registry).unwrap(), Some(real));
        assert!(validate_repo_path_in(&some(&other), &registry).unwrap_err().contains("not a repository Tracewise knows"));
        // a link that points at an unregistered folder is judged by where it leads
        let link = tmp.join("link");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&other, &link).unwrap();
            assert!(validate_repo_path_in(&some(&link), &registry).is_err());
            let good = tmp.join("good-link");
            std::os::unix::fs::symlink(&known, &good).unwrap();
            assert!(validate_repo_path_in(&some(&good), &registry).is_ok());
        }
        assert!(validate_repo_path_in(&some(&tmp), &registry).is_err(), "a parent folder is not a repository");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn a_stuck_or_noisy_command_is_stopped() {
        use super::run_bounded;
        use std::process::Command;
        use std::time::Duration;
        let slow = run_bounded(Command::new("sleep").arg("5"), Duration::from_millis(200), 1024);
        assert!(slow.unwrap_err().contains("did not finish"));
        let noisy = run_bounded(&mut Command::new("yes"), Duration::from_secs(10), 1000);
        assert!(noisy.unwrap_err().contains("printed more than"));
        // a background process that keeps the pipe open must not hold the call (a daemon started by the command)
        let started = std::time::Instant::now();
        let bg = run_bounded(Command::new("sh").args(["-c", "sleep 6 & echo hi"]), Duration::from_secs(1), 1024).unwrap();
        assert!(started.elapsed() < Duration::from_secs(3), "waited {:?}", started.elapsed());
        assert_eq!(bg.stdout, "hi\n");
        assert!(bg.stderr.contains("incomplete"));
        let fine = run_bounded(Command::new("echo").arg("hi"), Duration::from_secs(5), 1024).unwrap();
        assert_eq!((fine.stdout.as_str(), fine.exit_code), ("hi\n", 0));
    }

    #[test]
    fn test_build_command_strips_agent_vars() {
        let dummy_bin = PathBuf::from("tracewise");
        let args = vec!["status".to_string(), "--json".to_string()];
        let cmd = build_tracewise_command(&dummy_bin, &args, None);

        assert_eq!(cmd.get_program(), "tracewise");
    }

    /// The calls the UI's action buttons make, against the real `tracewise` in a scratch repository.
    /// Skipped when no binary is found. Run it outside an agent session (agents are refused undo).
    #[test]
    fn action_commands_work_against_the_real_tracewise() {
        let Ok(bin) = super::locate_tracewise_binary() else { return };
        let dir = std::env::temp_dir().join(format!("tracewise-ui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sh = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            // `init` and `scan` only prepare the scratch repository; the UI itself may not call them.
            if !matches!(args[0].as_str(), "init" | "scan") {
                super::validate_tracewise_args(&args).expect("allow-listed");
            }
            let out = build_tracewise_command(&bin, &args, Some(&dir)).env("TRACEWISE_HOME", dir.join("home")).output().unwrap();
            (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).to_string())
        };
        let git = |args: &[&str]| assert!(std::process::Command::new("git").args(args).current_dir(&dir).status().unwrap().success());
        git(&["init", "-q"]);
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        assert_eq!(sh(&["init"]).0, 0);
        std::fs::write(dir.join("a.txt"), "two\n").unwrap();
        assert_eq!(sh(&["scan"]).0, 0);
        let (code, log) = sh(&["log", "--json"]);
        assert_eq!(code, 0);
        let rows: serde_json::Value = serde_json::from_str(&log).unwrap();
        let change = rows.as_array().unwrap().iter().find(|c| c["op"] == "modify").expect("a modify");
        let id = change["id"].as_str().unwrap();
        for args in [vec!["lock", "list", "--json"], vec!["ps", "--json"], vec!["pending", "--json"], vec!["verify", "--json"]] {
            let (code, out) = sh(&args);
            assert_eq!(code, 0, "{args:?}");
            serde_json::from_str::<serde_json::Value>(&out).unwrap_or_else(|e| panic!("{args:?} not JSON: {e}"));
        }
        assert_eq!(sh(&["undo", id, "--dry-run", "--json"]).0, 0);
        assert_eq!(sh(&["restore", "a.txt", "--at", id]).0, 0);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "two\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
