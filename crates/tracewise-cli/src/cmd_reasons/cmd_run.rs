//! Running commands with reasons and attribution: tracewise run.
use super::super::*;

pub(crate) fn run_cmd_run(repo_paths: &RepoPaths, why: String, command: Vec<String>) {
    if let Err(e) = validate_why(&why) {
        eprintln!("tracewise: {e}");
        process::exit(1);
    }

    if command.is_empty() {
        eprintln!("tracewise: no command specified to run");
        process::exit(1);
    }

    let db_path = repo_paths.data_dir().join("tracewise.db");
    if !db_path.exists() {
        eprintln!("tracewise: not initialized; run tracewise init first");
        process::exit(1);
    }
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("tracewise: failed to open db: {e}");
            process::exit(1);
        }
    };

    let who = tracewise_proc::who(None, None);
    let actor = resolve_run_actor(&db, who);

    let now_utc = tracewise_core::clock::now();
    let now = now_utc.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let expires = (now_utc + chrono::Duration::hours(4)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let intent_id = ulid::Ulid::new().to_string();
    let intent = Intent {
        id: intent_id.clone(),
        kind: "run".to_string(),
        actor: actor.clone(),
        paths: vec!["*".to_string(), "**/*".to_string()],
        why,
        task: None,
        created: now.clone(),
        expires,
        ended: 0,
    };
    let _ = db.insert_intent(&intent);

    check_run_locks(&db, repo_paths, &actor, &command, &intent_id);

    let exit_code = execute_child_command(&command);

    record_run_changes(&db, repo_paths, &actor, &command, &now, &intent_id, exit_code);
    process::exit(exit_code);
}

fn check_run_locks(db: &Database, repo_paths: &RepoPaths, actor: &Actor, command: &[String], intent_id: &str) {
    for p in predict_written_files(&command.join(" ")) {
        let Some(rel) = normalize_and_validate_path(&p.to_string_lossy(), &repo_paths.work_tree) else {
            continue;
        };
        if let Ok(Some(lock)) = db.get_lock_for_path(&rel) {
            if !lock.holder.is_same(actor) {
                let holder = lock.holder.name.as_deref().unwrap_or("another session");
                if actor.kind == "agent" {
                    eprintln!("tracewise: refused: {rel} is held by {holder}");
                    let _ = db.end_intent_by_id(intent_id);
                    process::exit(1);
                }
                eprintln!("tracewise: warning: {rel} is held by {holder}");
            }
        }
    }
}

fn execute_child_command(command: &[String]) -> i32 {
    let program = &command[0];
    let cmd_args = &command[1..];
    match std::process::Command::new(program)
        .args(cmd_args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
    {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("tracewise: failed to execute {program}: {e}");
            1
        }
    }
}

fn record_run_changes(
    db: &Database,
    repo_paths: &RepoPaths,
    actor: &Actor,
    command: &[String],
    now: &str,
    intent_id: &str,
    exit_code: i32,
) {
    let cmd_text = command.join(" ");
    let change_ids = if tracewise_core::predict::only_reads(&cmd_text) {
        Vec::new()
    } else {
        let (branch, head) = get_git_info(repo_paths);
        let files = walk_baseline_files(&repo_paths.work_tree);
        let scan_opts = ScanOptions {
            source: "run".to_string(),
            actor: Some(actor.clone()),
            confidence: None,
            predicted_paths: None,
            window_start: Some(now.to_string()),
            ..Default::default()
        };
        scan_repository(&repo_paths.work_tree, &repo_paths.data_dir(), &branch, &head, &scan_opts, &files).unwrap_or_default()
    };

    let cmd_id = ulid::Ulid::new().to_string();
    let end_time = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let window = Window {
        id: cmd_id.clone(),
        kind: "run".to_string(),
        actor: actor.clone(),
        paths: Vec::new(),
        cwd: Some(repo_paths.work_tree.to_string_lossy().to_string()),
        start: now.to_string(),
        end: Some(end_time.clone()),
        command: Some(cmd_text.clone()),
    };
    let reads_only = tracewise_core::predict::only_reads(&cmd_text);
    let _ = tracewise_core::attribute_window(db, &window, actor, false, reads_only);

    let cmd_record = CommandRecord {
        id: cmd_id.clone(),
        kind: "run".to_string(),
        text: cmd_text,
        cwd: Some(repo_paths.work_tree.to_string_lossy().to_string()),
        actor: actor.clone(),
        start: now.to_string(),
        end: end_time,
        exit_code,
        changes: change_ids.clone(),
    };
    let _ = db.insert_command(&cmd_record);
    let _ = db.link_changes_to_command(&change_ids, &cmd_id);

    for cid in &change_ids {
        let _ = db.link_intent_to_change(cid, intent_id);
    }
    let _ = db.end_intent_by_id(intent_id);
}

pub(crate) fn resolve_run_actor(db: &Database, who: tracewise_proc::ProcWhoResult) -> Actor {
    let mut actor = Actor { kind: who.kind, name: if who.name.is_empty() { None } else { Some(who.name) }, session: who.session };

    if who.source == "process" && actor.kind == "agent" {
        let open_window = db.get_open_windows().ok().and_then(|windows| {
            windows
                .into_iter()
                .find(|w| (w.kind == "agent_command" || w.kind == "agent_edit") && (w.actor.name == actor.name || actor.name.is_none()))
        });
        if let Some(w) = open_window {
            if actor.session.is_none() {
                actor.session = w.actor.session;
            }
        } else {
            let had_closed_windows = actor.name.as_deref().is_some_and(|name| db.has_closed_windows_for_agent(name).unwrap_or(false));
            if had_closed_windows {
                let human_name = std::env::var("TRACEWISE_HUMAN")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| std::env::var("USER").ok().filter(|s| !s.trim().is_empty()))
                    .unwrap_or_else(|| "unknown".to_string());
                actor = Actor { kind: "human".to_string(), name: Some(human_name), session: None };
            }
        }
    }
    actor
}

#[cfg(test)]
#[path = "test_cmd_run.rs"]
mod tests;
