//! Commands: Strict (status, install, uninstall) — the opt-in sensor that names the process behind every write.
//!
//! Install runs `sudo` in the user's own terminal (Tracewise never sees the password) and puts the sensor in a
//! root-owned folder, because a root program must not be run from a place the user can rewrite.
use super::*;

const LABEL: &str = "dev.tracewise.sensor";
const SENSOR_DIR: &str = "/usr/local/libexec/tracewise";

/// `TRACEWISE_STRICT_PREFIX` moves every system path under a folder (tests); empty on a real machine.
fn root(path: &str) -> String {
    format!("{}{}", std::env::var("TRACEWISE_STRICT_PREFIX").unwrap_or_default(), path)
}

fn sensor_path() -> String {
    root(&format!("{SENSOR_DIR}/tracewise-sensor"))
}

fn plist_path() -> String {
    root(&format!("/Library/LaunchDaemons/{LABEL}.plist"))
}

fn plist_text(socket: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\
  <key>Label</key><string>{LABEL}</string>\n\
  <key>ProgramArguments</key>\n  <array>\n    <string>{SENSOR_DIR}/tracewise-sensor</string>\n    <string>--socket</string>\n    <string>{}</string>\n  </array>\n\
  <key>RunAtLoad</key><true/>\n\
  <key>KeepAlive</key><true/>\n\
  <key>ProcessType</key><string>Background</string>\n\
  <key>StandardErrorPath</key><string>/var/log/tracewise-sensor.log</string>\n\
</dict>\n</plist>\n",
        socket.display()
    )
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// What `tracewise strict status` reports; also the strict part of `tracewise doctor`.
pub fn status_json() -> serde_json::Value {
    let home = tracewise_core::get_tracewise_home();
    let file: serde_json::Value = fs::read_to_string(home.join("sensor.status.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(serde_json::Value::Null);
    let installed = Path::new(&sensor_path()).is_file() && Path::new(&plist_path()).is_file();
    // the daemon refreshes the file every few seconds; an old file means no daemon is listening
    let fresh = file["updated"].as_str().and_then(|s| s.parse::<u64>().ok()).is_some_and(|t| now_secs().saturating_sub(t) < 30);
    let connected = fresh && file["connected"].as_bool().unwrap_or(false);
    let running = connected && file["running"].as_bool().unwrap_or(false);
    let rejected = fresh && !connected && file["rejected"].as_u64().unwrap_or(0) > 0;
    let reason = if rejected {
        Some("rejected-peer".to_string())
    } else if !installed && !connected {
        Some("not-installed".to_string())
    } else if !fresh {
        Some("daemon-not-running".to_string())
    } else if !connected {
        Some("sensor-not-connected".to_string())
    } else {
        file["reason"].as_str().map(String::from)
    };
    serde_json::json!({
        "installed": installed,
        "connected": connected,
        "active": fresh && file["active"].as_bool().unwrap_or(false),
        "running": running,
        "reason": reason,
        "macos": file["macos"],
        "events": file["events"],
        "forwarded": file["forwarded"],
        "skipped": file["skipped"],
        "rejected": file["rejected"],
        "last_event": file["last_event"],
        "eslogger": Path::new("/usr/bin/eslogger").is_file(),
    })
}

pub(crate) fn run(json: bool, command: Commands) {
    let Commands::Strict { action } = command else {
        unreachable!("dispatched to the wrong group");
    };
    match action {
        StrictAction::Status => handle_status(json),
        StrictAction::Install { dry_run, events } => handle_install(dry_run, &events),
        StrictAction::Uninstall { dry_run } => handle_uninstall(dry_run),
    }
}

fn handle_status(json: bool) {
    let st = status_json();
    if json {
        println!("{st}");
    } else {
        let yes = |k: &str| {
            if st[k].as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            }
        };
        println!("installed: {}", yes("installed"));
        println!("connected: {}", yes("connected"));
        println!("running:   {}", yes("running"));
        if let Some(r) = st["reason"].as_str() {
            println!("reason:    {r}");
        }
        println!("events:    {} seen by the daemon", st["events"].as_u64().unwrap_or(0));
    }
}

fn handle_install(dry_run: bool, events: &str) {
    let socket = tracewise_core::get_tracewise_home().join("sensor.sock");
    let steps: Vec<Vec<String>> = vec![
        vec!["sudo", "install", "-d", "-o", "root", "-g", "wheel", "-m", "755", &root(SENSOR_DIR)].into_iter().map(String::from).collect(),
        vec!["sudo", "install", "-o", "root", "-g", "wheel", "-m", "755", "<tracewise-sensor>", &sensor_path()]
            .into_iter()
            .map(String::from)
            .collect(),
        vec!["sudo", "install", "-o", "root", "-g", "wheel", "-m", "644", "<plist>", &plist_path()].into_iter().map(String::from).collect(),
        tracewise_platform::service::system_bootout_command(LABEL),
        tracewise_platform::service::system_bootstrap_command(&plist_path()),
    ];
    let fda = format!("{SENSOR_DIR}/tracewise-sensor");
    if dry_run {
        println!(
            "{}",
            serde_json::json!({
                "steps": steps,
                "plist": plist_text(&socket),
                "full_disk_access": fda,
                "events": events,
            })
        );
        return;
    }
    refuse_for_agents("install");
    if events != "light" && events != "full" {
        eprintln!("tracewise: --events must be light or full");
        process::exit(1);
    }
    let _ = fs::write(tracewise_core::get_tracewise_home().join("sensor-events"), format!("{events}\n"));
    let me = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
    let Some(sensor) = me.as_ref().and_then(|p| p.parent()).map(|d| d.join("tracewise-sensor")).filter(|p| p.is_file()) else {
        eprintln!("tracewise: tracewise-sensor must be installed next to tracewise");
        process::exit(1);
    };
    let plist_tmp = std::env::temp_dir().join(format!("{LABEL}.{}.plist", std::process::id()));
    if fs::write(&plist_tmp, plist_text(&socket)).is_err() {
        eprintln!("tracewise: cannot write a temporary file");
        process::exit(1);
    }
    println!("Installing the strict sensor. sudo will ask for your password in this terminal.");
    for step in &steps {
        let args: Vec<String> = step
            .iter()
            .map(|a| match a.as_str() {
                "<tracewise-sensor>" => sensor.to_string_lossy().to_string(),
                "<plist>" => plist_tmp.to_string_lossy().to_string(),
                _ => a.clone(),
            })
            .collect();
        let ok = std::process::Command::new(&args[0]).args(&args[1..]).status().map(|s| s.success()).unwrap_or(false);
        if !ok && !args.contains(&"bootout".to_string()) {
            eprintln!("tracewise: step failed: {}", args.join(" "));
            let _ = fs::remove_file(&plist_tmp);
            process::exit(1);
        }
    }
    let _ = fs::remove_file(&plist_tmp);
    println!();
    println!("Installed. One more thing only you can do:");
    println!("  System Settings > Privacy & Security > Full Disk Access > + > add  {fda}");
    println!("  (press Cmd-Shift-G in the file picker to type the path), then switch it on.");
    println!("Then run `tracewise daemon start` and `tracewise strict status`: the sensor starts by itself");
    println!("while an agent works and stops when it is done.");
}

fn handle_uninstall(dry_run: bool) {
    let steps: Vec<Vec<String>> = vec![
        tracewise_platform::service::system_bootout_command(LABEL),
        vec!["sudo".to_string(), "rm".to_string(), "-f".to_string(), plist_path()],
        vec!["sudo".to_string(), "rm".to_string(), "-rf".to_string(), root(SENSOR_DIR)],
    ];
    if dry_run {
        println!("{}", serde_json::json!({ "steps": steps }));
        return;
    }
    refuse_for_agents("uninstall");
    for step in &steps {
        let _ = std::process::Command::new(&step[0]).args(&step[1..]).status();
    }
    let _ = fs::remove_file(tracewise_core::get_tracewise_home().join("sensor.status.json"));
    println!("Removed the sensor. You can also remove it from Full Disk Access.");
}

/// A root program is installed by a person, in their own terminal.
fn refuse_for_agents(what: &str) {
    let who = tracewise_proc::who(None, None);
    if who.kind == "agent" {
        eprintln!(
            "tracewise: strict {what} is refused for an agent ({}): it needs your password and your decision. Run it yourself in a terminal.",
            who.name
        );
        process::exit(1);
    }
}
