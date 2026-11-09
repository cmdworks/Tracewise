//! File locking commands: lock list, lock release, lock wait.
use super::super::*;
use tracewise_core::model::LockRecord;

pub(crate) fn run_lock(json: bool, repo_paths: &RepoPaths, action: LockAction) {
    let db_path = repo_paths.data_dir().join("trail.db");
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("failed to open database: {e}");
            process::exit(1);
        }
    };

    match action {
        LockAction::List => lock_list(&db, repo_paths, json),
        LockAction::Release { path, all } => lock_release(&db, repo_paths, json, path, all),
        LockAction::Wait { path, agent, session, timeout } => lock_wait(&db, repo_paths, json, path, agent, session, timeout),
    }
}

fn lock_list(db: &Database, repo_paths: &RepoPaths, json: bool) {
    let _ = db.clean_expired_locks(&repo_paths.work_tree);
    let mut locks = db.get_active_locks().unwrap_or_default();
    let stuck = db.find_stuck_immutable_files(&repo_paths.work_tree);
    for s in &stuck {
        if !locks.iter().any(|l| l.path == *s) {
            locks.push(LockRecord {
                path: s.clone(),
                holder: Actor { kind: "none".to_string(), name: None, session: None },
                since: String::new(),
                expires: "none".to_string(),
                intent: None,
            });
        }
    }
    if json {
        println!("{}", serde_json::to_string(&locks).unwrap_or_default());
    } else {
        for lock in locks {
            let holder_name = lock.holder.name.as_deref().unwrap_or(&lock.holder.kind);
            println!("{}\t{}\t{}", lock.path, holder_name, lock.expires);
        }
    }
}

fn lock_release(db: &Database, repo_paths: &RepoPaths, json: bool, path: Option<String>, all: bool) {
    // An agent releases only its own locks; a person releases anyone's.
    let caller = Actor::from_env().filter(|a| a.kind == "agent");
    let is_mine = |l: &LockRecord| caller.as_ref().is_none_or(|c| l.holder.is_same(c));
    if all {
        let count = match &caller {
            None => db.release_all_locks_with_flags(&repo_paths.work_tree).unwrap_or(0),
            Some(_) => db
                .get_active_locks()
                .unwrap_or_default()
                .iter()
                .filter(|l| is_mine(l))
                .filter(|l| db.release_lock_path_with_flags(&l.path, &repo_paths.work_tree).unwrap_or(false))
                .count(),
        };
        if json {
            println!("{}", serde_json::json!({ "released": count }));
        } else {
            println!("released {} lock(s)", count);
        }
    } else if let Some(p) = path {
        let rel_p = match normalize_and_validate_path(&p, &repo_paths.work_tree) {
            Some(rp) => rp,
            None => {
                if json {
                    println!("{}", serde_json::json!({ "released": false }));
                } else {
                    println!("no lock on {}", p);
                }
                process::exit(0);
            }
        };
        if let Ok(Some(l)) = db.get_lock_for_path(&rel_p) {
            if !is_mine(&l) {
                eprintln!(
                    "trail: {} is held by {}; an agent cannot release another actor's lock (use `trail lock wait`)",
                    rel_p,
                    l.holder.name.as_deref().unwrap_or(&l.holder.kind)
                );
                process::exit(1);
            }
        }
        let released = db.release_lock_path_with_flags(&rel_p, &repo_paths.work_tree).unwrap_or(false);
        if json {
            println!("{}", serde_json::json!({ "released": released }));
        } else if released {
            println!("released {}", rel_p);
        } else {
            println!("no lock on {}", rel_p);
        }
    } else {
        eprintln!("trail: specify a path to release or --all");
        process::exit(1);
    }
}

fn lock_wait(
    db: &Database,
    repo_paths: &RepoPaths,
    json: bool,
    path: String,
    agent: Option<String>,
    session: Option<String>,
    timeout: u64,
) {
    let rel_p = match normalize_and_validate_path(&path, &repo_paths.work_tree) {
        Some(rp) => rp,
        None => {
            // Unsafe path: fail open (allow) but take no lock, set no flag, record nothing
            if json {
                println!("{}", serde_json::json!({ "acquired": true }));
            } else {
                println!("acquired lock on {}", path);
            }
            process::exit(0);
        }
    };
    let who = tracewise_proc::who(None, None);
    // The agent's own markers carry the session its hooks report; the process chain may not.
    let marker = Actor::from_env().filter(|a| a.kind == "agent");
    let actor_name = agent.or(marker.as_ref().and_then(|m| m.name.clone())).or(if who.name.is_empty() { None } else { Some(who.name) });
    let actor_session = session.or(marker.and_then(|m| m.session)).or(who.session);
    let actor = Actor { kind: if actor_name.is_some() { "agent".to_string() } else { who.kind }, name: actor_name, session: actor_session };

    let ttl = crate::util::lock_ttl_secs();
    let start = std::time::Instant::now();
    let timeout_duration = std::time::Duration::from_secs(timeout);

    loop {
        let _ = db.clean_expired_locks(&repo_paths.work_tree);
        let holder = match db.acquire_lock(&rel_p, &actor, None, ttl) {
            Ok(None) => {
                if json {
                    println!("{}", serde_json::json!({ "acquired": true }));
                } else {
                    println!("acquired lock on {}", rel_p);
                }
                process::exit(0);
            }
            Ok(Some(conflict)) => conflict.holder,
            Err(e) => {
                eprintln!("trail: failed to acquire lock: {e}");
                process::exit(1);
            }
        };

        if start.elapsed() >= timeout_duration {
            if json {
                println!("{}", serde_json::json!({ "acquired": false, "holder": holder }));
            } else {
                eprintln!("trail: timeout waiting for lock on {}", rel_p);
            }
            process::exit(1);
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
