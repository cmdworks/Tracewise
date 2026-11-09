//! Commit gating and commit reason linking.
use super::*;

pub(crate) fn check_commit_gate(repo_paths_before: &Option<RepoPaths>, git_args: &[String]) {
    let is_commit = git_args.iter().any(|a| a == "commit");
    if !is_commit {
        return;
    }

    let skip_requested = std::env::var("TRAIL_SKIP_GATE").as_deref() == Ok("1");
    // A person may skip the commit gate; an agent may not.
    let skip_who = if skip_requested { Some(tracewise_proc::who(None, None)) } else { None };
    let skip_gate = skip_who.as_ref().is_some_and(|w| w.kind != "agent");
    if skip_gate {
        return;
    }

    let Some(ref repo_paths) = repo_paths_before else {
        return;
    };
    let db_path = repo_paths.data_dir().join("trail.db");
    if !db_path.exists() {
        return;
    }
    let Ok(db) = Database::open(&db_path) else {
        return;
    };

    if let Some(w) = skip_who.as_ref().filter(|w| w.kind == "agent") {
        let actor = Actor { kind: w.kind.clone(), name: Some(w.name.clone()), session: w.session.clone() };
        crate::util::record_ignored_switch(&db, &actor, "TRAIL_SKIP_GATE=1");
    }

    let (branch, _head) = get_git_info(repo_paths);
    if let Ok(changes) = db.get_agent_changes_for_branch(&branch) {
        let mut uncovered = Vec::new();
        for c in changes {
            if c.intent.is_none() && c.actor.session.is_some() {
                let agent = c.actor.name.as_deref().unwrap_or("");
                let session = c.actor.session.as_deref();
                if let Ok(Some(intent)) = db.find_any_covering_intent(agent, session, &c.path) {
                    let _ = db.link_intent_to_change(&c.id, &intent.id);
                } else if !uncovered.contains(&c.path) {
                    uncovered.push(c.path);
                }
            }
        }
        if !uncovered.is_empty() {
            eprintln!("trail: {} agent change(s) have no reason:", uncovered.len());
            for p in &uncovered {
                eprintln!("  {p}");
            }
            process::exit(1);
        }
    }
}

pub(crate) fn link_commit_subject(real_git: &Path, work_tree: &Path, db: &Database) {
    let run = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new(real_git).arg("-C").arg(work_tree).args(args).env("TRAIL_GIT_DEPTH", "1").output().ok()?;
        if out.status.success() {
            Some(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            None
        }
    };
    let subject = match run(&["log", "-1", "--format=%s"]) {
        Some(s) => s.trim().to_string(),
        None => return,
    };
    if tracewise_core::validate_why(&subject).is_err() {
        return;
    }
    let files: std::collections::HashSet<String> = match run(&["show", "--name-only", "--format=", "HEAD"]) {
        Some(s) => s.lines().map(|l| l.trim().to_string()).collect(),
        None => return,
    };
    let pending = db.list_pending_changes().unwrap_or_default();
    let mine: Vec<_> = pending.into_iter().filter(|c| c.actor.kind == "human" && files.contains(&c.path)).collect();
    if mine.is_empty() {
        return;
    }
    let now = tracewise_core::clock::now();
    let intent = tracewise_core::Intent {
        id: ulid::Ulid::new().to_string(),
        actor: mine[0].actor.clone(),
        paths: mine.iter().map(|c| c.path.clone()).collect(),
        why: subject,
        task: None,
        created: now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        expires: (now + chrono::Duration::hours(4)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        kind: "commit".to_string(),
        ended: 1,
    };
    if db.insert_intent(&intent).is_ok() {
        for c in &mine {
            let _ = db.link_intent_to_change(&c.id, &intent.id);
        }
    }
}
