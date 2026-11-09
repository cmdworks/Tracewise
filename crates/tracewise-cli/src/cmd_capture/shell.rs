//! Shell integration: preexec, precmd, init, begin, end.
use super::super::*;
use super::commands::local_hhmm;

pub(crate) fn run_shell_action(_json: bool, repo_paths: &RepoPaths, action: ShellAction) {
    match action {
        ShellAction::Init { shell, .. } => shell_init(&shell),
        ShellAction::Preexec { id, command } => shell_preexec(repo_paths, id, command),
        ShellAction::Precmd { id, exit } => shell_precmd(repo_paths, id, exit),
        ShellAction::Begin { pid, command } => {
            if let Err(()) = fast_shell_begin(pid, command) {
                process::exit(1);
            }
        }
        ShellAction::End { pid, exit } => {
            fast_shell_end(pid, Some(exit));
        }
    }
}

fn shell_init(shell: &str) {
    match shell {
        "zsh" => {
            print!("{}", tracewise_platform::shell::zsh_interactive());
        }
        "fish" => {
            print!("{}", tracewise_platform::shell::fish_interactive());
        }
        "pwsh" | "powershell" => {
            print!("{}", tracewise_platform::shell::pwsh_interactive());
        }
        "nu" | "nushell" => {
            print!("{}", tracewise_platform::shell::nu_interactive());
        }
        _ => {
            eprintln!("trail: unsupported shell {shell}");
        }
    }
}

fn shell_preexec(repo_paths: &RepoPaths, id: String, command: Vec<String>) {
    let cmd_text = command.join(" ");
    let predicted = predict_written_files(&cmd_text);
    let rel_predicted: Vec<String> = predicted.into_iter().map(|p| normalize_path(&p.to_string_lossy(), &repo_paths.work_tree)).collect();

    let actor = get_human_actor();
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let window = Window {
        id: format!("shell_{id}"),
        kind: "shell".to_string(),
        actor,
        paths: rel_predicted,
        cwd: Some(repo_paths.work_tree.to_string_lossy().to_string()),
        start: now,
        end: None,
        command: Some(cmd_text),
    };

    let db_path = repo_paths.data_dir().join("trail.db");
    if db_path.exists() {
        if let Ok(db) = Database::open_hot(&db_path) {
            let _ = db.open_window(&window);
        }
    }
}

fn shell_precmd(repo_paths: &RepoPaths, id: String, exit: i32) {
    let db_path = repo_paths.data_dir().join("trail.db");
    if !db_path.exists() {
        return;
    }
    let mut db = match Database::open_hot(&db_path) {
        Ok(d) => d,
        Err(_) => return,
    };

    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let window_id = format!("shell_{id}");
    let closed_window = db.close_window_by_id(&window_id, &now).ok().flatten();

    let (cmd_text, window_start, actor, predicted_paths) = if let Some(w) = closed_window {
        let p_set: HashSet<String> = w.paths.into_iter().collect();
        (w.command.unwrap_or_default(), w.start, w.actor, p_set)
    } else {
        (String::new(), now.clone(), get_human_actor(), HashSet::new())
    };

    let (branch, head) = get_git_info(repo_paths);
    let scan_opts = ScanOptions {
        source: "shell".to_string(),
        actor: Some(actor.clone()),
        confidence: None,
        predicted_paths: Some(predicted_paths.clone()),
        window_start: Some(window_start.clone()),
        ..Default::default()
    };

    let change_ids = scan_shell_changes(repo_paths, &branch, &head, &scan_opts, &predicted_paths, &mut db);

    let mut change_ids = change_ids;
    let window = Window {
        id: window_id,
        kind: "shell".to_string(),
        actor: actor.clone(),
        paths: predicted_paths.iter().cloned().collect(),
        cwd: Some(repo_paths.work_tree.to_string_lossy().to_string()),
        start: window_start.clone(),
        end: Some(now.clone()),
        command: Some(cmd_text.clone()),
    };
    let reads_only = tracewise_core::predict::only_reads(&cmd_text);
    if let Ok(attr_ids) = tracewise_core::attribute_window(&db, &window, &actor, !predicted_paths.is_empty(), reads_only) {
        for cid in attr_ids {
            if !change_ids.contains(&cid) {
                change_ids.push(cid);
            }
        }
    }

    let cmd_id = ulid::Ulid::new().to_string();
    let failed_command_text = if exit != 0 { cmd_text.clone() } else { String::new() };
    let cmd_record = CommandRecord {
        id: cmd_id.clone(),
        kind: "shell".to_string(),
        text: cmd_text,
        cwd: Some(repo_paths.work_tree.to_string_lossy().to_string()),
        actor,
        start: window_start,
        end: now,
        exit_code: exit,
        changes: change_ids.clone(),
    };

    let _ = db.insert_command(&cmd_record);
    let _ = db.link_changes_to_command(&change_ids, &cmd_id);

    for cid in &change_ids {
        if let Ok(Some(ch)) = db.get_change(cid) {
            if let Some(conflict) = ch.lock_conflict {
                let name = conflict.get("name").and_then(|v| v.as_str()).unwrap_or("agent");
                eprintln!("warning: {} is held by {}", ch.path, name);
            }
        }
    }

    check_held_file_failure(&db, repo_paths, &failed_command_text);
    check_human_pending_reminder(&db, &id);
    prompt_line(&db, repo_paths, &id);
}

/// Opt-in prompt line (`[shell] prompt_line = "on"` or `TRAIL_PROMPT_LINE=1`): one line on stderr when someone else changed files
/// since this shell's previous prompt, e.g. `trail: 3 changes in 2 files by claude, cursor since your last prompt`.
/// The time of the previous prompt is a tiny per-shell file; nothing is written to the database.
fn prompt_line(db: &Database, repo_paths: &RepoPaths, shell_id: &str) {
    let on = std::env::var("TRAIL_PROMPT_LINE")
        .ok()
        .or_else(|| tracewise_core::reasons::config_value(&repo_paths.work_tree, "shell", "prompt_line"));
    if !matches!(on.as_deref().map(str::trim), Some("1" | "on")) {
        return;
    }
    let safe: String = shell_id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let file = repo_paths.data_dir().join(format!("prompt-seen-{safe}"));
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let since = fs::read_to_string(&file).ok();
    let _ = fs::write(&file, &now);
    let Some(since) = since.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) else { return };
    let Ok(changes) = db.list_changes_since(&since) else { return };
    let others: Vec<_> = changes.iter().filter(|c| c.actor.kind != "human" && c.time.start.as_str() > since.as_str()).collect();
    if others.is_empty() {
        return;
    }
    let mut files: Vec<&str> = others.iter().map(|c| c.path.as_str()).collect();
    files.sort();
    files.dedup();
    let mut who: Vec<&str> = others.iter().map(|c| c.actor.name.as_deref().unwrap_or(c.actor.kind.as_str())).collect();
    who.sort();
    who.dedup();
    eprintln!(
        "trail: {} change{} in {} file{} by {} since your last prompt",
        others.len(),
        if others.len() == 1 { "" } else { "s" },
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        who.join(", ")
    );
}

fn scan_shell_changes(
    repo_paths: &RepoPaths,
    branch: &str,
    head: &str,
    scan_opts: &ScanOptions,
    predicted_paths: &HashSet<String>,
    db: &mut Database,
) -> Vec<String> {
    if !predicted_paths.is_empty() {
        let path_bufs: Vec<PathBuf> = predicted_paths.iter().map(PathBuf::from).collect();
        scan_specific_paths_with_db(&repo_paths.work_tree, &repo_paths.data_dir(), branch, head, scan_opts, &path_bufs, db)
            .unwrap_or_default()
    } else {
        let files = walk_baseline_files(&repo_paths.work_tree);
        scan_repository(&repo_paths.work_tree, &repo_paths.data_dir(), branch, head, scan_opts, &files).unwrap_or_default()
    }
}

fn check_held_file_failure(db: &Database, repo_paths: &RepoPaths, failed_command_text: &str) {
    for token in failed_command_text.split_whitespace() {
        let token = token.trim_matches(|c| c == '"' || c == '\'');
        if token.starts_with('-') {
            continue;
        }
        let Some(rel) = util::normalize_and_validate_path(token, &repo_paths.work_tree) else {
            continue;
        };
        if let Ok(Some(lock)) = db.get_lock_for_path(&rel) {
            let who = lock.holder.name.as_deref().unwrap_or("another agent");
            let session = lock.holder.session.as_deref().map(|s| format!(", session {s}")).unwrap_or_default();
            let until = chrono::DateTime::parse_from_rfc3339(&lock.expires).map(local_hhmm).unwrap_or_else(|_| lock.expires.clone());
            eprintln!(
                "trail: {rel} is held by {who}{session} until {until}; `trail lock list` shows it, `trail lock release {rel}` frees it"
            );
        }
    }
}

fn check_human_pending_reminder(db: &Database, shell_id: &str) {
    let pending = db.list_pending_changes().unwrap_or_default();
    let human_pending_count = pending.into_iter().filter(|c| c.actor.kind == "human").count();
    let prev_count = db.get_shell_pending_count(shell_id).unwrap_or(None).unwrap_or(0);
    let _ = db.set_shell_pending_count(shell_id, human_pending_count);
    if human_pending_count > prev_count && human_pending_count > 0 {
        eprintln!("trail: {} change(s) need a reason: trail note --last \"...\"", human_pending_count);
    }
}
