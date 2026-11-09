use std::fs;
use std::path::{Path, PathBuf};
use tracewise_core::model::Actor;
use tracewise_git::RepoPaths;

/// `--since`: an RFC 3339 time, or a whole number and a unit (`s`, `m`, `h`, `d`, `w`) counted back from now.
pub fn parse_since(since: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let s = since.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    let (num, unit) = s.split_at(s.len().checked_sub(1)?);
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: i64 = num.parse().ok().filter(|n| *n <= 100_000)?;
    let span = match unit {
        "s" => chrono::Duration::seconds(n),
        "m" => chrono::Duration::minutes(n),
        "h" => chrono::Duration::hours(n),
        "d" => chrono::Duration::days(n),
        "w" => chrono::Duration::weeks(n),
        _ => return None,
    };
    Some(tracewise_core::clock::now() - span)
}

/// Exits with a message when `--since` is given and is not a valid duration or time.
pub fn require_valid_since(since: Option<&str>) {
    if let Some(v) = since.filter(|v| parse_since(v).is_none()) {
        eprintln!("trail: invalid --since '{v}': use a duration like 30m, 2h, 1d or 1w, or an RFC 3339 time");
        std::process::exit(2);
    }
}

pub fn normalize_path(path_str: &str, work_tree: &Path) -> String {
    tracewise_platform::paths::normalize_path(path_str, work_tree)
}

/// Normalises an external path relative to the work tree and verifies it is safe
/// (relative to work tree, no `..`, not outside the tree, not inside `.git`).
pub fn normalize_and_validate_path(path_str: &str, work_tree: &Path) -> Option<String> {
    if path_str.is_empty() || path_str.contains('\0') {
        return None;
    }
    let home_expanded;
    let path_str = match (path_str.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) if !home.is_empty() => {
            home_expanded = format!("{home}/{rest}");
            home_expanded.as_str()
        }
        _ => path_str,
    };
    let p = Path::new(path_str);
    let rel_str = if p.is_absolute() {
        if let Ok(stripped) = p.strip_prefix(work_tree) {
            stripped.to_string_lossy().to_string()
        } else if let Ok(wt_canon) = work_tree.canonicalize() {
            if let Ok(stripped) = p.strip_prefix(&wt_canon) {
                stripped.to_string_lossy().to_string()
            } else if let Ok(p_canon) = p.canonicalize() {
                if let Ok(stripped) = p_canon.strip_prefix(&wt_canon) {
                    stripped.to_string_lossy().to_string()
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        }
    } else {
        let mut clean_components = Vec::new();
        for c in p.components() {
            match c {
                std::path::Component::Normal(n) => clean_components.push(n),
                std::path::Component::CurDir => continue,
                std::path::Component::ParentDir => {
                    clean_components.pop()?;
                }
                _ => return None,
            }
        }
        if clean_components.is_empty() {
            return None;
        }
        let mut pb = std::path::PathBuf::new();
        for comp in clean_components {
            pb.push(comp);
        }
        pb.to_string_lossy().to_string()
    };

    if tracewise_core::share::is_safe_rel_path(&rel_str) {
        Some(rel_str)
    } else {
        None
    }
}

/// Seconds a lock lives without a heartbeat (any hook event of its session renews it). A finished or crashed agent
/// frees its files after this long; its Stop hook frees them at once.
pub fn lock_ttl_secs() -> u64 {
    std::env::var("TRAIL_LOCK_TTL_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(120)
}

pub fn get_human_actor() -> Actor {
    if let Some(actor) = Actor::from_env() {
        return actor;
    }
    human_actor_from_system()
}

/// The person at this machine (TRAIL_HUMAN, git's user.name, USER), ignoring any agent marker in the environment:
/// for a daemon that was started from an agent's shell.
pub fn human_actor_from_system() -> Actor {
    let name = std::env::var("TRAIL_HUMAN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::process::Command::new("git").args(["config", "user.name"]).output().ok().and_then(|out| {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
                None
            })
        })
        .or_else(|| std::env::var("USER").ok().filter(|s| !s.trim().is_empty()))
        .unwrap_or_else(|| "human".to_string());

    Actor { kind: "human".to_string(), name: Some(name), session: None }
}

pub fn is_daemon_running() -> bool {
    let trail_home = tracewise_core::get_trail_home();
    let pid_file = trail_home.join("daemon.pid");
    if let Ok(content) = fs::read_to_string(&pid_file) {
        if let Ok(pid) = content.trim().parse::<i32>() {
            return tracewise_platform::signal::is_daemon_pid(pid);
        }
    }
    false
}

/// `root` joined with a recorded path, or None when the path could reach outside `root` (or into `.git`).
pub fn safe_join(root: &std::path::Path, rel: &str) -> Option<std::path::PathBuf> {
    tracewise_core::share::is_safe_rel_path(rel).then(|| root.join(rel))
}

/// The repository's own work tree: for a linked worktree that is the main one (the data folder is shared).
pub fn main_work_tree(repo: &RepoPaths) -> PathBuf {
    if repo.git_common_dir.file_name().is_some_and(|n| n == ".git") {
        if let Some(parent) = repo.git_common_dir.parent() {
            return parent.to_path_buf();
        }
    }
    repo.work_tree.clone()
}

/// `Some(original)` when this folder is an unadopted copy of another repository folder.
pub fn copy_of(repo: &RepoPaths) -> Option<PathBuf> {
    let data_dir = repo.data_dir();
    if !data_dir.join("trail.db").exists() {
        return None;
    }
    match tracewise_core::identity::check_copy(&data_dir, &main_work_tree(repo)) {
        tracewise_core::identity::CopyState::Copy { original } => Some(original),
        _ => None,
    }
}

fn lexical(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// For an agent's `cp`/`mv`: the destination when something from inside `work_tree` is copied or moved to a place
/// outside it that no tracked repository covers, so the copy would have no history.
pub fn export_destination(work_tree: &Path, cwd: &Path, tool: &str, args: &[String]) -> Option<PathBuf> {
    let (sources, dest) = tracewise_shim::copy_operands(tool, args)?;
    let abs = |p: &str| {
        let p = Path::new(p);
        lexical(&if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) })
    };
    let work_tree = work_tree.canonicalize().unwrap_or_else(|_| work_tree.to_path_buf());
    let dest = abs(&dest);
    if dest.starts_with(&work_tree) || !sources.iter().any(|s| abs(s).starts_with(&work_tree)) {
        return None;
    }
    let tracked_there = dest.ancestors().any(|a| a.join(".git").join("trail").join("trail.db").exists());
    (!tracked_there).then_some(dest)
}

/// Writes "something left the repository" into the command history.
pub fn record_export(db: &tracewise_core::Database, actor: &Actor, tool: &str, args: &[String], dest: &Path) {
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let _ = db.insert_command(&tracewise_core::model::CommandRecord {
        id: ulid::Ulid::new().to_string(),
        kind: "export".to_string(),
        text: format!("export: {tool} {} (to {})", args.join(" "), dest.display()),
        cwd: std::env::current_dir().ok().map(|d| d.to_string_lossy().to_string()),
        actor: actor.clone(),
        start: now.clone(),
        end: now,
        exit_code: 0,
        changes: Vec::new(),
    });
}

/// Reasons mode for `actor`. An agent's `TRAIL_REASONS` is ignored, and the attempt is recorded as a command.
pub fn reasons_mode_for(db: &tracewise_core::Database, root: &std::path::Path, actor: &Actor) -> tracewise_core::ReasonsMode {
    let mode = tracewise_core::get_reasons_mode_for(root, actor);
    if actor.kind == "agent" {
        if let Ok(v) = std::env::var("TRAIL_REASONS") {
            if tracewise_core::ReasonsMode::parse(v.trim()) != Some(mode) {
                record_ignored_switch(db, actor, &format!("TRAIL_REASONS={v}"));
            }
        }
    }
    mode
}

/// Writes "an agent tried to use a gate switch" into the command history.
pub fn record_ignored_switch(db: &tracewise_core::Database, actor: &Actor, what: &str) {
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let _ = db.insert_command(&tracewise_core::model::CommandRecord {
        id: ulid::Ulid::new().to_string(),
        kind: "switch".to_string(),
        text: format!("{what} ignored: set by an agent"),
        cwd: std::env::current_dir().ok().map(|d| d.to_string_lossy().to_string()),
        actor: actor.clone(),
        start: now.clone(),
        end: now,
        exit_code: 0,
        changes: Vec::new(),
    });
}

#[cfg(test)]
#[path = "test_util.rs"]
mod tests;
