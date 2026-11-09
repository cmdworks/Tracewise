//! Miscellaneous reasons/doctor commands: adopt-copy, ps, background, doctor.
use super::super::*;

pub(crate) fn run_adopt_copy(json: bool, repo_paths: &RepoPaths) {
    let data_dir = repo_paths.data_dir();
    if !matches!(
        tracewise_core::identity::check_copy(&data_dir, &main_work_tree(repo_paths)),
        tracewise_core::identity::CopyState::Copy { .. }
    ) {
        eprintln!("tracewise: this repository is not a copy of another one (nothing to adopt)");
        process::exit(1);
    }
    match tracewise_core::identity::adopt_copy(&data_dir, &main_work_tree(repo_paths)) {
        Ok((previous, repo_id, device)) => {
            if json {
                println!("{}", serde_json::json!({ "repo_id": repo_id, "previous": previous, "device": device }));
            } else {
                println!("adopted: new id {repo_id}, pushes as device {device}");
            }
        }
        Err(e) => {
            eprintln!("tracewise: adopt-copy failed: {e}");
            process::exit(1);
        }
    }
}

pub(crate) fn run_ps(json: bool, repo_paths: &RepoPaths) {
    let db_path = repo_paths.data_dir().join("tracewise.db");
    let windows = Database::open_hot(&db_path).and_then(|db| db.list_background_windows()).unwrap_or_default();
    if json {
        let list: Vec<_> = windows
            .iter()
            .map(|w| {
                serde_json::json!({
                    "id": w.id,
                    "actor": w.actor,
                    "command": w.command,
                    "start": w.start,
                    "expires": w.end,
                })
            })
            .collect();
        println!("{}", serde_json::Value::Array(list));
    } else {
        for w in &windows {
            println!("{} {} {} (until {})", w.id, w.actor, w.command.as_deref().unwrap_or(""), w.end.as_deref().unwrap_or(""));
        }
    }
}

pub(crate) fn run_background(json: bool, repo_paths: &RepoPaths, action: BackgroundAction) {
    match action {
        BackgroundAction::Stop { id } => {
            let db_path = repo_paths.data_dir().join("tracewise.db");
            let stopped = Database::open_hot(&db_path).and_then(|db| db.stop_background_window(&id)).unwrap_or(false);
            if json {
                println!("{}", serde_json::json!({ "stopped": stopped }));
            } else if stopped {
                println!("stopped {id}");
            } else {
                eprintln!("tracewise: no live background window {id}");
            }
            if !stopped {
                process::exit(1);
            }
        }
    }
}

/// How well the last `days` days of changes are attributed: counts by confidence and by actor kind (baseline rows excluded).
fn attribution_summary(db_path: &std::path::Path, days: i64) -> Option<serde_json::Value> {
    let since = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let changes = Database::open_hot(db_path).ok()?.list_changes_since(&since).ok()?;
    let (mut confidence, mut kinds) = (std::collections::BTreeMap::<String, u64>::new(), std::collections::BTreeMap::<String, u64>::new());
    let real: Vec<_> = changes.iter().filter(|c| c.op != "baseline").collect();
    for c in &real {
        *confidence.entry(c.confidence.clone()).or_default() += 1;
        *kinds.entry(c.actor.kind.clone()).or_default() += 1;
    }
    Some(serde_json::json!({"days": days, "changes": real.len(), "confidence": confidence, "actor_kind": kinds}))
}

pub(crate) fn run_doctor(json: bool, repo_paths: &RepoPaths) {
    let cov = crate::cmd_agents::coverage(repo_paths);
    let data_dir = repo_paths.data_dir();
    let db_path = data_dir.join("tracewise.db");
    let mut problems: Vec<String> = Vec::new();
    let stuck = if db_path.exists() {
        match Database::open_hot(&db_path) {
            Ok(db) => {
                let _ = db.clean_expired_locks(&repo_paths.work_tree);
                db.find_stuck_immutable_files(&repo_paths.work_tree)
            }
            Err(e) => {
                problems.push(format!("cannot open {}: {e}", db_path.display()));
                Vec::new()
            }
        }
    } else {
        if data_dir.is_dir() {
            problems.push(format!("{} exists but tracewise.db cannot be read there (permissions on the folder?)", data_dir.display()));
        }
        Vec::new()
    };

    let attribution = if db_path.exists() { attribution_summary(&db_path, 7) } else { None };
    let prune_candidates = if db_path.exists() { crate::cmd_history::prune::candidates(repo_paths) } else { 0 };
    let profile = tracewise_core::profile::ProfileConfig::from_root(&repo_paths.work_tree);
    let sharing = tracewise_core::share::sharing_warnings(&repo_paths.work_tree, &data_dir);
    let deprecated_lock_flag = tracewise_core::reasons::config_value(&repo_paths.work_tree, "locks", "flag").is_some();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": stuck.is_empty() && problems.is_empty(),
                "profile": profile.description(),
                "problems": problems,
                "sharing_warnings": sharing,
                "attribution": attribution,
                "prune_candidates": prune_candidates,
                "stuck_immutable": stuck,
                "immutable_without_lock": stuck,
                "coverage": {
                    "agents": cov.agents.iter().cloned().collect::<std::collections::BTreeMap<_, _>>(),
                    "missing_events": cov.missing_events.iter().cloned().collect::<std::collections::BTreeMap<_, _>>(),
                    "shell": cov.shell,
                    "shims": cov.shims,
                    "git_wrapper": cov.git_wrapper,
                    "daemon": cov.daemon,
                    "strict": crate::cmd_strict::status_json(),
                }
            })
        );
    } else {
        println!("profile: {}", profile.description());
        if deprecated_lock_flag {
            println!("tracewise: warning: `[locks] flag` is no longer supported");
        }
        let config_files = [tracewise_core::get_tracewise_home().join("config.toml"), repo_paths.work_tree.join(".tracewiseconfig")];
        for file in &config_files {
            for w in std::fs::read_to_string(file).map(|c| tracewise_core::profile::ProfileConfig::config_warnings(&c)).unwrap_or_default()
            {
                println!("tracewise: warning: {}: {w}", file.display());
            }
        }
        if stuck.is_empty() && problems.is_empty() {
            println!("tracewise doctor: ok (no stuck immutable files)");
        }
        for p in &problems {
            println!("tracewise: PROBLEM: {p}");
        }
        for w in &sharing {
            println!("tracewise: warning: {w}");
        }
        for p in &stuck {
            println!("tracewise: warning: {p} is immutable with no live lock; run `tracewise lock release --all` to clear");
        }
        if let Some(a) = attribution.as_ref().filter(|a| a["changes"].as_u64().unwrap_or(0) > 0) {
            let total = a["changes"].as_u64().unwrap_or(1).max(1);
            let share = |k: &str| a["confidence"][k].as_u64().unwrap_or(0) * 100 / total;
            println!(
                "attribution (last {} days): {total} changes; exact {}%, likely {}%, ambiguous {}%, unknown {}%",
                a["days"],
                share("exact"),
                share("likely"),
                share("ambiguous"),
                share("unknown")
            );
            if share("unknown") > 30 {
                println!("  many changes have no named actor: install the missing agent hooks (see coverage below)");
            }
        }
        if prune_candidates > 0 {
            println!("prune candidates: {prune_candidates} record(s) under ignored paths; see `tracewise prune --dry-run`");
        }
        let yn = |b: bool| if b { "yes" } else { "NO" };
        println!("coverage:");
        for (name, hooks) in &cov.agents {
            println!("  agent {name}: hooks {}", yn(*hooks));
            if let Some((_, missing)) = cov.missing_events.iter().find(|(a, m)| a == name && !m.is_empty()) {
                println!(
                    "    missing hook events: {} (changes by {name} are attributed less often; run `tracewise agent install {name}`)",
                    missing.join(", ")
                );
            }
        }
        println!("  shell hook: {}", yn(cov.shell));
        println!("  shims first on PATH: {}", yn(cov.shims));
        println!("  git wrapper first on PATH: {}", yn(cov.git_wrapper));
        println!("  daemon running: {}", yn(cov.daemon));
        let strict = crate::cmd_strict::status_json();
        println!(
            "  strict sensor: {}",
            match (strict["running"].as_bool(), strict["reason"].as_str()) {
                (Some(true), _) => "running".to_string(),
                (_, Some(r)) => format!("not running ({r})"),
                _ => "idle (starts while an agent works)".to_string(),
            }
        );
    }
}
