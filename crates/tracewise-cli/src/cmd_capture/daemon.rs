//! Daemon control and event processing.
use super::super::*;
use super::commands::relative_to;

/// The pid is alive and is `tracewise-watchd` (a pid file can be stale, and the number can belong to something else).
pub(crate) fn daemon_pid_is_alive(pid: u32) -> bool {
    tracewise_platform::signal::is_daemon_pid(pid as i32)
}

/// Adds the repository we are in (if it has Tracewise data) to the list the daemon watches; true when it was new.
pub(crate) fn register_repo(repo: Option<&RepoPaths>) -> bool {
    match repo {
        Some(r) if r.data_dir().join("tracewise.db").is_file() => tracewise_core::repos::register(&r.work_tree, &r.data_dir()),
        _ => false,
    }
}

pub(crate) fn daemon_global(json: bool, action: DaemonAction, repo: Option<&RepoPaths>) {
    match action {
        DaemonAction::Run => daemon_run_action(repo),
        DaemonAction::Start => daemon_start_action(repo),
        DaemonAction::Stop => daemon_stop_action(),
        DaemonAction::Status => daemon_status_action(json),
        DaemonAction::Install { no_load } => daemon_install_action(json, no_load),
        DaemonAction::Uninstall => daemon_uninstall_action(json),
        DaemonAction::ScanPaths | DaemonAction::Active | DaemonAction::Sweep => {
            unreachable!("these need a repository")
        }
    }
}

/// Keeps `daemon.log` from growing without end: over 10 MB it becomes `daemon.log.1` (older copies shift up, three are kept) (J10).
fn rotate_log(tracewise_home: &std::path::Path) {
    let log = tracewise_home.join("daemon.log");
    if fs::metadata(&log).map(|m| m.len()).unwrap_or(0) <= 10 * 1024 * 1024 {
        return;
    }
    let numbered = |n: u32| tracewise_home.join(format!("daemon.log.{n}"));
    let _ = fs::remove_file(numbered(3));
    for n in (1..3).rev() {
        let _ = fs::rename(numbered(n), numbered(n + 1));
    }
    let _ = fs::rename(&log, numbered(1));
}

fn daemon_run_action(repo: Option<&RepoPaths>) {
    let tracewise_home = tracewise_core::get_tracewise_home();
    let _ = tracewise_platform::private::create_dir_all(&tracewise_home);
    rotate_log(&tracewise_home);
    let pid_file = tracewise_home.join("daemon.pid");
    let my_pid = std::process::id();
    if let Some(other) =
        fs::read_to_string(&pid_file).ok().and_then(|t| t.trim().parse::<u32>().ok()).filter(|p| *p != my_pid && daemon_pid_is_alive(*p))
    {
        register_repo(repo);
        tracewise_core::repos::wake_daemon();
        eprintln!("tracewise: a daemon is already running (pid {other})");
        return;
    }
    let _ = tracewise_platform::private::write(&pid_file, my_pid.to_string());
    register_repo(repo);

    let watchd = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .and_then(|p| p.parent().map(|d| d.join("tracewise-watchd")))
        .filter(|p| p.is_file());
    let Some(watchd) = watchd else {
        eprintln!("tracewise: tracewise-watchd must be installed next to tracewise");
        let _ = fs::remove_file(&pid_file);
        process::exit(1);
    };
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(watchd).exec();
    eprintln!("tracewise: cannot start tracewise-watchd: {err}");
    let _ = fs::remove_file(&pid_file);
    process::exit(1);
}

fn daemon_start_action(repo: Option<&RepoPaths>) {
    if register_repo(repo) {
        tracewise_core::repos::wake_daemon();
    }
    // ten `daemon start` at once must give one daemon: the check and the spawn happen under an exclusive file lock (K13)
    let _ = tracewise_platform::private::create_dir_all(tracewise_core::get_tracewise_home());
    let start_lock =
        fs::OpenOptions::new().create(true).append(true).open(tracewise_core::get_tracewise_home().join("daemon.start.lock")).ok();
    if let Some(f) = &start_lock {
        let _ = f.lock();
    }
    if is_daemon_running() {
        println!("tracewise daemon is already running");
        return;
    }
    let env = crate::setup::Env::current();
    if crate::setup::login_item_installed(&env) {
        crate::setup::login_item_load(&env);
        println!("tracewise daemon started (login item)");
        return;
    }
    let self_exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("tracewise: failed to get current executable: {e}");
            process::exit(1);
        }
    };
    let tracewise_home = tracewise_core::get_tracewise_home();
    let _ = tracewise_platform::private::create_dir_all(&tracewise_home);
    rotate_log(&tracewise_home);
    let log_path = tracewise_home.join("daemon.log");
    let log_file = match fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("tracewise: cannot open daemon log: {e}");
            process::exit(1);
        }
    };
    let log_err = match log_file.try_clone() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("tracewise: cannot clone daemon log handle: {e}");
            process::exit(1);
        }
    };

    let child = match std::process::Command::new(&self_exe)
        .args(["daemon", "run"])
        .stdin(std::process::Stdio::null())
        .stdout(log_file)
        .stderr(log_err)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tracewise: failed to spawn daemon: {e}");
            process::exit(1);
        }
    };

    let _ = tracewise_platform::private::write(tracewise_home.join("daemon.pid"), child.id().to_string());
    println!("tracewise daemon started");
}

fn daemon_stop_action() {
    if crate::setup::login_item_installed(&crate::setup::Env::current()) {
        crate::setup::login_item_unload(&crate::setup::Env::current());
    }
    let tracewise_home = tracewise_core::get_tracewise_home();
    let pid_file = tracewise_home.join("daemon.pid");
    if let Ok(content) = fs::read_to_string(&pid_file) {
        if let Ok(pid) = content.trim().parse::<i32>() {
            let _ = tracewise_platform::signal::terminate_daemon(pid);
            for _ in 0..20 {
                if !daemon_pid_is_alive(pid as u32) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        let _ = fs::remove_file(&pid_file);
    }
    for leftover in ["sensor.sock", "sensor.status.json"] {
        let _ = fs::remove_file(tracewise_home.join(leftover));
    }
    println!("tracewise daemon stopped");
}

fn daemon_status_action(json: bool) {
    let running = is_daemon_running();
    let home = tracewise_core::get_tracewise_home();
    let pid = fs::read_to_string(home.join("daemon.pid")).ok().and_then(|t| t.trim().parse::<u32>().ok()).filter(|_| running);
    let uptime = pid
        .and_then(|_| fs::metadata(home.join("daemon.pid")).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs());
    let repos: Vec<String> = tracewise_core::repos::list().iter().map(|r| r.work_tree.display().to_string()).collect();
    if json {
        println!(
            "{}",
            serde_json::json!({ "daemon": if running { "running" } else { "stopped" }, "pid": pid, "uptime_secs": uptime, "repositories": repos })
        );
    } else {
        let detail = match (pid, uptime) {
            (Some(p), Some(u)) => format!(" (pid {p}, up {})", human_duration(u)),
            _ => String::new(),
        };
        println!("daemon: {}{detail}", if running { "running" } else { "stopped" });
        println!("repositories: {}", repos.len());
        repos.iter().for_each(|r| println!("  {r}"));
    }
}

fn human_duration(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m {}s", secs / 60, secs % 60),
        _ => format!("{}h {}m", secs / 3600, secs % 3600 / 60),
    }
}

fn daemon_install_action(json: bool, no_load: bool) {
    let env = crate::setup::Env::current();
    if let Err(e) = crate::setup::login_item_write(&env) {
        eprintln!("tracewise: {e}");
        process::exit(1);
    }
    if !no_load {
        crate::setup::stop_manual_daemon(&env);
        std::thread::sleep(std::time::Duration::from_millis(500));
        crate::setup::login_item_load(&env);
    }
    let plist = env.agent_plist();
    if json {
        println!("{}", serde_json::json!({ "plist": plist.to_string_lossy() }));
    } else {
        println!("Installed {}", plist.display());
    }
}

fn daemon_uninstall_action(json: bool) {
    let env = crate::setup::Env::current();
    if crate::setup::login_item_installed(&env) {
        crate::setup::login_item_unload(&crate::setup::Env::current());
        let _ = fs::remove_file(env.agent_plist());
    }
    if json {
        println!("{}", serde_json::json!({ "uninstalled": true }));
    } else {
        println!("Uninstalled daemon");
    }
}

pub(crate) fn daemon_scan_paths(repo_paths: &RepoPaths) {
    use std::io::Read;
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    let data_dir = repo_paths.data_dir();
    let (branch, head) = get_git_info(repo_paths);
    let db = Database::open_hot(&data_dir.join("tracewise.db")).ok();
    let windows = db.as_ref().and_then(|d| d.get_open_windows().ok()).unwrap_or_default();
    let actor_for = |agent: &Option<String>| match agent {
        Some(name) => {
            let mut sessions: Vec<String> = windows
                .iter()
                .filter(|w| w.actor.kind == "agent" && w.actor.name.as_deref() == Some(name))
                .filter_map(|w| w.actor.session.clone())
                .collect();
            sessions.sort();
            sessions.dedup();
            match sessions.as_slice() {
                [one] => Actor::agent(name, one),
                _ => Actor { kind: "agent".to_string(), name: Some(name.clone()), session: None },
            }
        }
        None => human_actor_from_system(),
    };
    let mut groups: Vec<(Option<Actor>, Vec<PathBuf>)> = Vec::new();
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(path) = v["path"].as_str().map(PathBuf::from) else {
            continue;
        };
        let rel = if path.is_relative() {
            path
        } else {
            match relative_to(&path, &repo_paths.work_tree) {
                Some(r) => r,
                None => continue,
            }
        };
        let hinted = v["hinted"].as_bool().unwrap_or(false);
        let agent = v["agent"].as_str().map(String::from);
        if matches!(v["kind"].as_str(), Some("setflags" | "setextattr" | "deleteextattr")) {
            if let (true, Some(db)) = (hinted, db.as_ref()) {
                let culprit = actor_for(&agent);
                let _ = db.name_recent_tamper(&rel.to_string_lossy(), &culprit);
            }
            continue;
        }
        let who = hinted.then(|| actor_for(&agent));
        match groups.iter_mut().find(|(a, _)| *a == who) {
            Some((_, paths)) => paths.push(rel),
            None => groups.push((who, vec![rel])),
        }
    }
    for (who, paths) in groups {
        if let (Some(actor), Some(db)) = (&who, db.as_ref()) {
            let since = (tracewise_core::clock::now() - chrono::Duration::seconds(10)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            for u in db.find_unknown_changes_since(&since).unwrap_or_default() {
                if paths.iter().any(|p| p.to_string_lossy() == u.path) {
                    let _ = db.upgrade_change_actor(&u.id, actor, "exact", "sensor");
                }
            }
        }
        let unhinted = who.is_none();
        let options = match who {
            Some(actor) => ScanOptions {
                source: "sensor".to_string(),
                actor: Some(actor),
                confidence: Some("exact".to_string()),
                ..Default::default()
            },
            None => ScanOptions { source: "watch".to_string(), ..Default::default() },
        };
        let ids =
            tracewise_core::scan_specific_paths(&repo_paths.work_tree, &data_dir, &branch, &head, &options, &paths).unwrap_or_default();
        // what only file events saw, while an agent was working here, may be a write no hook explained
        if let (true, Some(db)) = (unhinted, db.as_ref()) {
            crate::unhooked::note(db, &data_dir, &ids);
            crate::bulk::note(db, &ids);
        }
    }
}

pub(crate) fn daemon_active(repo_paths: &RepoPaths) {
    let active = Database::open_hot(&repo_paths.data_dir().join("tracewise.db"))
        .map(|db| {
            let two_hours_ago =
                (tracewise_core::clock::now() - chrono::Duration::hours(2)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let open_agent =
                db.get_open_windows().unwrap_or_default().iter().any(|w| w.kind.starts_with("agent") && w.start > two_hours_ago);
            open_agent || !db.list_background_windows().unwrap_or_default().is_empty()
        })
        .unwrap_or(false);
    println!("{}", if active { 1 } else { 0 });
}

pub(crate) fn daemon_sweep(repo_paths: &RepoPaths) {
    let held = Database::open_hot(&repo_paths.data_dir().join("tracewise.db"))
        .map(|db| {
            let _ = db.clean_expired_locks(&repo_paths.work_tree);
            // restores wanted agent hooks and records a restart of the daemon; prints nothing (the output below is read)
            crate::self_heal::on_sweep(repo_paths, &db);
            db.get_active_locks().map(|l| l.len()).unwrap_or(1)
        })
        .unwrap_or(1);
    println!("{held}");
}
