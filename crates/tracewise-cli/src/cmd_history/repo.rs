//! Repository lifecycle and maintenance: init, status, scan, gc, verify.
use super::*;

pub(crate) fn run_init(json: bool, repo_paths: &RepoPaths) {
    let data_dir = repo_paths.data_dir();
    if let Err(e) = fs::create_dir_all(&data_dir) {
        eprintln!("tracewise: failed to create data directory: {e}");
        process::exit(1);
    }

    let files = walk_baseline_files(&repo_paths.work_tree);
    let (branch, head) = get_git_info(repo_paths);
    if let Err(e) = record_baseline(&repo_paths.work_tree, &data_dir, &branch, &head, &files) {
        eprintln!("tracewise: baseline recording failed: {e}");
        process::exit(1);
    }
    // the daemon watches every registered repository: tell it about this one
    if tracewise_core::repos::register(&repo_paths.work_tree, &data_dir) {
        tracewise_core::repos::wake_daemon();
    }

    if json {
        let doc = serde_json::json!({
            "repo": repo_paths.work_tree.to_string_lossy(),
            "data_dir": data_dir.to_string_lossy(),
            "files": files.len(),
        });
        println!("{}", doc);
    } else {
        println!("Initialized tracewise in {} ({} baseline files)", data_dir.display(), files.len());
    }
}

pub(crate) fn run_status(json: bool, repo_paths: &RepoPaths) {
    let db_path = repo_paths.data_dir().join("tracewise.db");
    let (change_count, lock_count) = if db_path.is_file() {
        Database::open(&db_path).map(|db| (db.count_changes().unwrap_or(0), db.count_active_locks().unwrap_or(0))).unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    let share_cfg = tracewise_core::share::read_tracewiseconfig(&repo_paths.work_tree);
    let share_json = if let Some(ref s) = share_cfg {
        serde_json::json!({
            "remote": s.remote,
            "level": s.level,
        })
    } else {
        serde_json::Value::Null
    };

    let daemon_status = if is_daemon_running() { "running" } else { "stopped" };

    let profile = tracewise_core::profile::ProfileConfig::from_root(&repo_paths.work_tree);

    if json {
        let doc = serde_json::json!({
            "repo": repo_paths.work_tree.to_string_lossy(),
            "data_dir": repo_paths.data_dir().to_string_lossy(),
            "changes": change_count,
            "locks": lock_count,
            "daemon": daemon_status,
            "repo_id": tracewise_core::identity::repo_id(
                &repo_paths.data_dir(),
                &main_work_tree(repo_paths),
            ),
            "share": share_json,
            "profile": {
                "name": profile.description(),
                "core": profile.core,
                "coordination": profile.coordination,
                "policy": profile.policy,
                "strict": profile.strict,
                "share": profile.share,
            },
        });
        println!("{}", doc);
    } else {
        println!("repo:     {}", repo_paths.work_tree.display());
        println!("data_dir: {}", repo_paths.data_dir().display());
        println!("profile:  {}", profile.description());
        println!("changes:  {}", change_count);
        println!("locks:    {}", lock_count);
        println!("daemon:   {}", daemon_status);
        if let Some(ref s) = share_cfg {
            println!("share:    {} ({})", s.remote, s.level);
        } else {
            println!("share:    null");
        }
    }
}

pub(crate) fn run_scan(json: bool, repo_paths: &RepoPaths) {
    let files = walk_baseline_files(&repo_paths.work_tree);
    let (branch, head) = get_git_info(repo_paths);
    let options = ScanOptions::default();
    match scan_repository(&repo_paths.work_tree, &repo_paths.data_dir(), &branch, &head, &options, &files) {
        Ok(changes) => {
            if json {
                let doc = serde_json::json!({ "changes": changes });
                println!("{}", doc);
            } else if changes.is_empty() {
                println!("No changes detected.");
            } else {
                println!("Recorded {} change(s):", changes.len());
                for id in changes {
                    println!("  {id}");
                }
            }
        }
        Err(e) => {
            eprintln!("tracewise: scan failed: {e}");
            process::exit(1);
        }
    }
}

pub(crate) fn run_gc(json: bool, repo_paths: &RepoPaths) {
    let data_dir = repo_paths.data_dir();
    let db_path = data_dir.join("tracewise.db");
    let mut db_gc_report = None;
    if db_path.exists() {
        if let Ok(db) = Database::open(&db_path) {
            if let Ok(report) = db.gc(Some(&repo_paths.work_tree)) {
                db_gc_report = Some(report);
            }
        }
    }

    let blobs = BlobStore::new(&data_dir);
    let packed = blobs.gc();
    tracewise_platform::private::strip_tree(&data_dir); // git's pack files are read-only for everyone, not private
    match packed {
        Ok(mut report) => {
            if let Some(dbrep) = db_gc_report {
                report.commands_pruned = Some(dbrep.commands_pruned);
            }
            if json {
                println!("{}", serde_json::to_string(&report).unwrap_or_default());
            } else {
                println!("Packed {} object(s): {} bytes -> {} bytes", report.packed, report.bytes_before, report.bytes_after);
                if let Some(n) = report.commands_pruned {
                    if n > 0 {
                        println!("Pruned {n} expired command record(s)");
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("tracewise: gc failed: {e}");
            process::exit(1);
        }
    }
}

pub(crate) fn run_verify(json: bool, repo_paths: &RepoPaths) {
    let db_path = repo_paths.data_dir().join("tracewise.db");
    if !db_path.exists() {
        if repo_paths.data_dir().join("chain-head").exists() {
            eprintln!("tracewise: FAILED: tracewise.db is missing but its chain-head file exists; the history was deleted");
        } else {
            eprintln!("tracewise: not initialized; run tracewise init first");
        }
        process::exit(1);
    }
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("tracewise: failed to open db: {e}");
            process::exit(1);
        }
    };
    let report = match db.verify_hash_chain() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("tracewise: verify error: {e}");
            process::exit(1);
        }
    };
    let missing = match db.changes_with_missing_blobs(&tracewise_core::blob::BlobStore::new(&repo_paths.data_dir())) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("tracewise: verify error: {e}");
            process::exit(1);
        }
    };
    if json {
        let mut value = serde_json::to_value(&report).unwrap_or_default();
        value["missing_blobs"] = serde_json::json!(missing);
        println!("{value}");
    } else if report.ok && missing.is_empty() {
        println!("ok ({} changes verified)", report.changes);
    } else if report.ok {
        eprintln!("FAILED (chain ok, but {} changes point to a missing file version, first: {})", missing.len(), missing[0]);
    } else {
        eprintln!("FAILED ({} changes verified, first bad: {})", report.changes, report.first_bad.as_deref().unwrap_or("unknown"));
    }
    if !report.ok || !missing.is_empty() {
        process::exit(1);
    }
}
