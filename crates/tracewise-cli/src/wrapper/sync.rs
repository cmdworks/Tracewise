//! Post-command repository scanning, flag restoration, and clone synchronization.
use super::*;

pub(crate) fn dirty_paths(git: &Path, work_tree: &Path) -> Vec<PathBuf> {
    let out = std::process::Command::new(git)
        .arg("-C")
        .arg(work_tree)
        .args(["status", "--porcelain", "-uall"])
        .env("TRACEWISE_GIT_DEPTH", "1")
        .output();
    let Ok(o) = out else { return Vec::new() };
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| l.len() > 3)
        .map(|l| {
            let p = l[3..].trim();
            PathBuf::from(p.split(" -> ").last().unwrap_or(p))
        })
        .collect()
}

pub(crate) fn handle_clone_sync(git_args: &[String], exit_code: i32, current_dir: &Path) {
    let is_clone = git_args.first().map(|s| s.as_str()) == Some("clone") || git_args.iter().any(|s| s == "clone");
    if is_clone {
        if exit_code == 0 {
            if let Some(target) = find_clone_target(git_args) {
                let target_dir = if target.is_absolute() { target } else { current_dir.join(target) };
                if let Some(cloned_repo) = RepoPaths::discover(&target_dir) {
                    if cloned_repo.work_tree.join(".tracewiseconfig").is_file() {
                        let data_dir = cloned_repo.data_dir();
                        let _ = fs::create_dir_all(&data_dir);
                        let db_path = data_dir.join("tracewise.db");
                        let _ = Database::open(&db_path);
                        if let Err(e) = tracewise_core::share::fetch(&cloned_repo.work_tree, &data_dir) {
                            eprintln!("tracewise: not syncing history: {e}");
                        }
                    }
                }
            }
        }
        process::exit(exit_code);
    }
}

pub(crate) fn scan_git_changes(
    real_git: &Path,
    repo_paths: &RepoPaths,
    subcmd: &str,
    head: &str,
    pre: &GitPreState,
    git_args: &[String],
    scan_opts: &ScanOptions,
) -> Vec<String> {
    let data_dir = repo_paths.data_dir();
    let (branch, head_info) = get_git_info(repo_paths);
    let old_head = &pre.old_head;
    if !is_working_tree_changing_subcmd(subcmd) {
        Vec::new()
    } else if (subcmd == "checkout" || subcmd == "switch" || subcmd == "reset")
        && !old_head.is_empty()
        && !head.is_empty()
        && old_head != head
    {
        scan_diff_tree(real_git, repo_paths, &data_dir, &branch, &head_info, scan_opts, &[old_head, head])
    } else if subcmd == "commit" {
        scan_diff_tree(real_git, repo_paths, &data_dir, &branch, &head_info, scan_opts, &["--root", "-m", "HEAD"])
    } else if !old_head.is_empty() && !head.is_empty() && old_head != head {
        scan_diff_tree(real_git, repo_paths, &data_dir, &branch, &head_info, scan_opts, &[old_head, head])
    } else {
        scan_status_changes(real_git, repo_paths, subcmd, pre, git_args, scan_opts)
    }
}

pub(crate) fn scan_diff_tree(
    real_git: &Path,
    repo_paths: &RepoPaths,
    data_dir: &Path,
    branch: &str,
    head: &str,
    scan_opts: &ScanOptions,
    extra_args: &[&str],
) -> Vec<String> {
    let mut cmd = std::process::Command::new(real_git);
    cmd.arg("-C")
        .arg(&repo_paths.work_tree)
        .args(["diff-tree", "-r", "--name-only", "--no-commit-id"])
        .args(extra_args)
        .env("TRACEWISE_GIT_DEPTH", "1");
    let Ok(o) = cmd.output() else {
        return Vec::new();
    };
    let paths: Vec<PathBuf> =
        String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim()).filter(|l| !l.is_empty()).map(PathBuf::from).collect();
    if !paths.is_empty() {
        tracewise_core::scan_specific_paths(&repo_paths.work_tree, data_dir, branch, head, scan_opts, &paths).unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub(crate) fn scan_status_changes(
    real_git: &Path,
    repo_paths: &RepoPaths,
    subcmd: &str,
    pre: &GitPreState,
    git_args: &[String],
    scan_opts: &ScanOptions,
) -> Vec<String> {
    let data_dir = repo_paths.data_dir();
    let (branch, head_info) = get_git_info(repo_paths);
    let out = std::process::Command::new(real_git)
        .arg("-C")
        .arg(&repo_paths.work_tree)
        .args(["status", "--porcelain", "-uall"])
        .env("TRACEWISE_GIT_DEPTH", "1")
        .output();
    let mut paths = pre.dirty_before.to_vec();
    for arg in git_args {
        if !arg.starts_with('-') && arg != subcmd {
            let candidate = repo_paths.work_tree.join(arg);
            if candidate.is_file() {
                paths.push(PathBuf::from(arg));
            }
        }
    }
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            // `XY path` or `XY old -> new`: the status letters are 2 columns, the first may be a space
            if let Some(p) = line.get(3..).map(str::trim).filter(|p| !p.is_empty()) {
                for part in p.split(" -> ") {
                    paths.push(PathBuf::from(part.trim_matches('"')));
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    if !paths.is_empty() {
        tracewise_core::scan_specific_paths(&repo_paths.work_tree, &data_dir, &branch, &head_info, scan_opts, &paths).unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub(crate) fn determine_git_actor(db: &Database) -> (Actor, String) {
    if let Some(a) = Actor::from_env() {
        (a, "exact".to_string())
    } else {
        let open_windows = db.get_open_windows().unwrap_or_default();
        if let Some(w) = open_windows.iter().find(|w| w.kind == "agent_command") {
            (w.actor.clone(), "likely".to_string())
        } else if let Some(w) = open_windows.iter().find(|w| w.kind == "shell") {
            (w.actor.clone(), "likely".to_string())
        } else {
            (get_human_actor(), "likely".to_string())
        }
    }
}

pub(crate) struct GitCommandRecordArgs<'a> {
    pub db: &'a Database,
    pub real_git: &'a Path,
    pub repo_paths: &'a RepoPaths,
    pub git_args: &'a [String],
    pub current_dir: &'a Path,
    pub actor: Actor,
    pub start_time: &'a str,
    pub exit_code: i32,
    pub change_ids: &'a [String],
}

pub(crate) fn record_mutating_git_command(args: GitCommandRecordArgs) {
    let cmd_id = ulid::Ulid::new().to_string();
    let cmd_record = CommandRecord {
        id: cmd_id.clone(),
        kind: "git".to_string(),
        text: format!("git {}", args.git_args.join(" ")),
        cwd: Some(args.current_dir.to_string_lossy().to_string()),
        actor: args.actor,
        start: args.start_time.to_string(),
        end: tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        exit_code: args.exit_code,
        changes: args.change_ids.to_vec(),
    };

    let _ = args.db.insert_command(&cmd_record);
    let _ = args.db.link_changes_to_command(args.change_ids, &cmd_id);

    let is_commit = args.git_args.iter().any(|a| a == "commit");
    if is_commit && args.exit_code == 0 {
        link_commit_subject(args.real_git, &args.repo_paths.work_tree, args.db);
    }
}
