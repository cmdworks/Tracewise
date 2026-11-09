//! Intent declaration and listing, plus pending changes query.
use super::super::*;

/// The agent a process chain found, with no session id (an intent without one covers every session of that agent).
fn agent_from_process(who: tracewise_proc::ProcWhoResult) -> Option<Actor> {
    if who.kind == "agent" {
        Some(Actor { kind: who.kind, name: Some(who.name), session: who.session })
    } else {
        None
    }
}

pub(crate) fn run_intent(json: bool, repo_paths: &RepoPaths, action: IntentAction) {
    match action {
        IntentAction::Add { why, task, agent, session, paths } => {
            if let Err(e) = validate_why(&why) {
                eprintln!("tracewise: {e}");
                process::exit(1);
            }

            let actor = if let Some(a) = Actor::from_env() {
                a
            } else if let Some(n) = agent {
                // a hook without a session id uses `default` (H04)
                Actor::agent(&n, session.as_deref().unwrap_or("default"))
            } else {
                // an agent without an environment marker (Codex) is found by its process, so its intent is not the person's
                agent_from_process(tracewise_proc::who(None, None)).unwrap_or_else(get_human_actor)
            };

            let rel_paths: Vec<String> = paths.iter().filter_map(|p| normalize_and_validate_path(p, &repo_paths.work_tree)).collect();

            let now_dt = tracewise_core::clock::now();
            let created = now_dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let expires = (now_dt + chrono::Duration::hours(4)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

            let intent = Intent {
                id: ulid::Ulid::new().to_string(),
                actor,
                paths: rel_paths,
                why,
                task,
                created,
                expires,
                kind: "declared".to_string(),
                ended: 0,
            };

            let db_path = repo_paths.data_dir().join("tracewise.db");
            if db_path.exists() {
                if let Ok(db) = Database::open(&db_path) {
                    let _ = db.insert_intent(&intent);
                }
            }

            if json {
                println!("{}", serde_json::json!({ "id": intent.id }));
            } else {
                println!("{}", intent.id);
            }
        }
        IntentAction::List { session } => {
            let db_path = repo_paths.data_dir().join("tracewise.db");
            let list = if db_path.exists() {
                Database::open(&db_path).and_then(|db| db.list_intents(session.as_deref())).unwrap_or_default()
            } else {
                Vec::new()
            };

            if json {
                println!("{}", serde_json::to_string(&list).unwrap_or_default());
            } else {
                for i in &list {
                    println!("{} {} ({})", i.id, i.why, i.kind);
                }
            }
        }
    }
}

pub(crate) fn run_pending(json: bool, repo_paths: &RepoPaths) {
    let db_path = repo_paths.data_dir().join("tracewise.db");
    let items =
        if db_path.exists() { Database::open(&db_path).and_then(|db| db.list_pending_changes()).unwrap_or_default() } else { Vec::new() };

    if json {
        println!("{}", serde_json::to_string(&items).unwrap_or_default());
    } else {
        for item in &items {
            println!("{} {} ({})", item.id, item.path, item.actor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who(kind: &str, name: &str) -> tracewise_proc::ProcWhoResult {
        tracewise_proc::ProcWhoResult { kind: kind.into(), name: name.into(), session: None, source: "process".into(), matched: None }
    }

    #[test]
    fn an_intent_from_a_codex_process_is_codexs_not_the_persons() {
        let actor = agent_from_process(who("agent", "codex")).unwrap();
        assert_eq!((actor.kind.as_str(), actor.name.as_deref(), actor.session), ("agent", Some("codex"), None));
    }

    #[test]
    fn no_agent_in_the_process_chain_leaves_the_person() {
        assert!(agent_from_process(who("human", "as")).is_none());
    }
}
