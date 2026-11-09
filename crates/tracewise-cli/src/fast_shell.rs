//! Hot path of `tracewise shell begin/end/preexec/precmd`: answers before clap is built.
use super::*;

pub(crate) fn try_fast_shell(args: &[String]) -> bool {
    let mut idx = 1;
    let mut custom_dir = None;
    if idx + 1 < args.len() && (args[idx] == "-C" || args[idx] == "--directory") {
        custom_dir = Some(&args[idx + 1]);
        idx += 2;
    }

    if idx >= args.len() || args[idx] != "shell" {
        return false;
    }
    idx += 1;
    if idx >= args.len() {
        return false;
    }

    let subcmd = &args[idx];
    idx += 1;

    if let Some(dir) = custom_dir {
        if std::env::set_current_dir(dir).is_err() {
            return false;
        }
    }

    let in_git = match std::env::current_dir() {
        Ok(d) => d.join(".git").exists() || RepoPaths::discover(&d).is_some(),
        Err(_) => false,
    };
    if !in_git && (subcmd == "begin" || subcmd == "end" || subcmd == "preexec" || subcmd == "precmd") {
        return true;
    }

    if subcmd == "begin" {
        let mut pid = None;
        let mut cmd_parts = Vec::new();
        let mut after_double_dash = false;

        while idx < args.len() {
            if after_double_dash {
                cmd_parts.push(args[idx].clone());
            } else if args[idx] == "--" {
                after_double_dash = true;
            } else if args[idx] == "--pid" && idx + 1 < args.len() {
                pid = args[idx + 1].parse::<u32>().ok();
                idx += 1;
            }
            idx += 1;
        }

        let pid = match pid {
            Some(p) => p,
            None => return false,
        };

        if let Err(()) = fast_shell_begin(pid, cmd_parts) {
            std::process::exit(1);
        }
        true
    } else if subcmd == "end" {
        let mut pid = None;
        let mut exit_code = None;

        while idx < args.len() {
            if args[idx] == "--pid" && idx + 1 < args.len() {
                pid = args[idx + 1].parse::<u32>().ok();
                idx += 1;
            } else if args[idx] == "--exit" && idx + 1 < args.len() {
                exit_code = args[idx + 1].parse::<i32>().ok();
                idx += 1;
            }
            idx += 1;
        }

        let pid = match pid {
            Some(p) => p,
            None => return false,
        };

        fast_shell_end(pid, exit_code);
        true
    } else {
        false
    }
}

pub(crate) fn fast_shell_begin(pid: u32, command: Vec<String>) -> Result<(), ()> {
    let current_dir = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };
    let dot_git = current_dir.join(".git");
    let (work_tree, data_dir) = if dot_git.is_dir() {
        (current_dir.clone(), dot_git.join("tracewise"))
    } else {
        match RepoPaths::discover(&current_dir) {
            Some(r) => (r.work_tree.clone(), r.data_dir()),
            None => return Ok(()),
        }
    };

    let db_path = data_dir.join("tracewise.db");
    if !db_path.exists() {
        return Ok(());
    }
    let db = match Database::open_hot(&db_path) {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };

    let open_windows = db.get_open_windows().unwrap_or_default();
    let has_shell = open_windows.iter().any(|w| w.kind == "shell");
    if has_shell {
        let chain = tracewise_proc::process_chain(pid);
        let ancestor_pids: HashSet<u32> = chain.iter().map(|p| p.pid).collect();
        for w in &open_windows {
            if w.kind == "shell" {
                if let Some(rest) = w.id.strip_prefix("shell_proc_") {
                    if let Ok(p) = rest.parse::<u32>() {
                        if ancestor_pids.contains(&p) {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    let cmd_text = command.join(" ");
    let predicted = predict_written_files(&cmd_text);
    let rel_predicted: Vec<String> = predicted.into_iter().map(|p| normalize_path(&p.to_string_lossy(), &work_tree)).collect();

    let who = tracewise_proc::who(Some(pid), None);
    let actor = Actor { kind: who.kind, name: if who.name.is_empty() { None } else { Some(who.name) }, session: who.session };

    handle_tracewise_why(&db, &actor, &rel_predicted);

    // Enforce mode refusal if any predicted path is uncovered
    check_reasons_enforce(&db, &work_tree, &actor, &rel_predicted)?;

    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let window = Window {
        id: format!("shell_proc_{pid}"),
        kind: "shell".to_string(),
        actor,
        paths: rel_predicted,
        cwd: Some(work_tree.to_string_lossy().to_string()),
        start: now,
        end: None,
        command: Some(cmd_text),
    };

    let _ = db.open_window(&window);
    Ok(())
}

fn handle_tracewise_why(db: &Database, actor: &Actor, rel_predicted: &[String]) {
    if let Ok(why_env) = std::env::var("TRACEWISE_WHY") {
        let trimmed = why_env.trim();
        if !trimmed.is_empty() {
            match validate_why(trimmed) {
                Ok(()) => {
                    let now_utc = tracewise_core::clock::now();
                    let now_str = now_utc.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                    let expires_str = (now_utc + chrono::Duration::hours(4)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                    let intent_paths =
                        if rel_predicted.is_empty() { vec!["*".to_string(), "**/*".to_string()] } else { rel_predicted.to_vec() };
                    let intent = Intent {
                        id: ulid::Ulid::new().to_string(),
                        kind: "shell".to_string(),
                        actor: actor.clone(),
                        paths: intent_paths,
                        why: trimmed.to_string(),
                        task: None,
                        created: now_str,
                        expires: expires_str,
                        ended: 0,
                    };
                    let _ = db.insert_intent(&intent);
                }
                Err(e) => {
                    eprintln!("tracewise: {}", e);
                }
            }
        }
    }
}

fn check_reasons_enforce(db: &Database, work_tree: &Path, actor: &Actor, rel_predicted: &[String]) -> Result<(), ()> {
    let reasons_mode = reasons_mode_for(db, work_tree, actor);
    if reasons_mode.should_enforce(actor) && !rel_predicted.is_empty() {
        for p in rel_predicted {
            let covered = if let Some(ref agent) = actor.name {
                db.find_covering_intent(agent, actor.session.as_deref(), p).ok().flatten().is_some()
            } else {
                false
            };
            if !covered {
                eprintln!("tracewise: no intent declared for {}; run tracewise intent add --why <reason> {}", p, p);
                return Err(());
            }
        }
    }
    Ok(())
}

pub(crate) fn fast_shell_end(pid: u32, exit: Option<i32>) {
    let current_dir = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => return,
    };
    let dot_git = current_dir.join(".git");
    let (repo_paths, work_tree, data_dir) = if dot_git.is_dir() {
        let repo = RepoPaths { work_tree: current_dir.clone(), git_dir: dot_git.clone(), git_common_dir: dot_git.clone() };
        (repo, current_dir.clone(), dot_git.join("tracewise"))
    } else {
        match RepoPaths::discover(&current_dir) {
            Some(r) => {
                let wt = r.work_tree.clone();
                let dd = r.data_dir();
                (r, wt, dd)
            }
            None => return,
        }
    };

    let db_path = data_dir.join("tracewise.db");
    if !db_path.exists() {
        return;
    }
    let mut db = match Database::open_hot(&db_path) {
        Ok(d) => d,
        Err(_) => return,
    };

    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let window_id = format!("shell_proc_{pid}");
    let closed_window = match db.close_window_by_id(&window_id, &now) {
        Ok(Some(w)) => w,
        _ => return,
    };

    let (cmd_text, window_start, actor, predicted_paths) = {
        let p_set: HashSet<String> = closed_window.paths.iter().cloned().collect();
        (closed_window.command.clone().unwrap_or_default(), closed_window.start.clone(), closed_window.actor.clone(), p_set)
    };

    let (branch, head) = get_git_info(&repo_paths);
    let scan_opts = ScanOptions {
        source: "shell".to_string(),
        actor: Some(actor.clone()),
        confidence: None,
        predicted_paths: Some(predicted_paths.clone()),
        window_start: Some(window_start.clone()),
        ..Default::default()
    };

    let mut change_ids = if !predicted_paths.is_empty() {
        let path_bufs: Vec<PathBuf> = predicted_paths.iter().map(PathBuf::from).collect();
        scan_specific_paths_with_db(&work_tree, &data_dir, &branch, &head, &scan_opts, &path_bufs, &mut db).unwrap_or_default()
    } else {
        let files = walk_baseline_files(&work_tree);
        scan_repository(&work_tree, &data_dir, &branch, &head, &scan_opts, &files).unwrap_or_default()
    };

    let reads_only = tracewise_core::predict::only_reads(&cmd_text);
    if let Ok(attr_ids) = tracewise_core::attribute_window(&db, &closed_window, &actor, !predicted_paths.is_empty(), reads_only) {
        for cid in attr_ids {
            if !change_ids.contains(&cid) {
                change_ids.push(cid);
            }
        }
    }

    let cmd_id = ulid::Ulid::new().to_string();
    let cmd_record = CommandRecord {
        id: cmd_id.clone(),
        kind: "shell".to_string(),
        text: cmd_text,
        cwd: Some(work_tree.to_string_lossy().to_string()),
        actor: actor.clone(),
        start: window_start,
        end: now,
        exit_code: exit.unwrap_or(0),
        changes: change_ids.clone(),
    };

    let _ = db.insert_command(&cmd_record);
    let _ = db.link_changes_to_command(&change_ids, &cmd_id);

    let _ = db.end_shell_intents(&actor);

    // Warn reminder for changes without a reason
    let reasons_mode = reasons_mode_for(&db, &work_tree, &actor);
    if reasons_mode.should_warn(&actor) {
        for cid in &change_ids {
            if let Ok(Some(ch)) = db.get_change(cid) {
                if ch.intent.is_none() {
                    eprintln!("tracewise: {} changed ({}) without a reason. Add one: tracewise note {} \"<why>\"", ch.path, ch.id, ch.id);
                }
            }
        }
    }
}
