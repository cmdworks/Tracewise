//! Command history and git wrapper install commands.
use super::super::*;

pub(crate) fn run_command_history(json: bool, repo_paths: &RepoPaths) {
    let db_path = repo_paths.data_dir().join("tracewise.db");
    let cmds = if db_path.exists() { Database::open(&db_path).and_then(|db| db.list_commands()).unwrap_or_default() } else { Vec::new() };

    if json {
        println!("{}", serde_json::to_string(&cmds).unwrap_or_default());
    } else {
        for c in &cmds {
            println!("{} {} ({})", c.start, c.text, c.kind);
        }
    }
}

pub(crate) fn run_git_wrapper_install(json: bool, dir: PathBuf) {
    let _ = fs::create_dir_all(&dir);
    let symlink_path = dir.join("git");
    let _ = fs::remove_file(&symlink_path);

    let self_exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("tracewise: failed to get current executable: {e}");
            process::exit(1);
        }
    };

    if let Err(e) = tracewise_platform::shims::create_shim_link(&self_exe, &symlink_path) {
        eprintln!("tracewise: failed to create symlink: {e}");
        process::exit(1);
    }

    if json {
        println!("{}", serde_json::json!({ "path": symlink_path.to_string_lossy() }));
    } else {
        println!("Installed git wrapper at {}", symlink_path.display());
    }
}

/// `path` relative to `work_tree`, whichever way either is spelled (`/private/var/x` and `/var/x` are one place).
pub(crate) fn relative_to(path: &Path, work_tree: &Path) -> Option<PathBuf> {
    tracewise_platform::paths::relative_to(path, work_tree)
}

/// "HH:MM" in the machine's local time, without chrono's time-zone database (which links CoreFoundation).
pub(crate) fn local_hhmm<Tz: chrono::TimeZone>(t: chrono::DateTime<Tz>) -> String {
    tracewise_platform::time::local_hhmm(t.timestamp())
}
