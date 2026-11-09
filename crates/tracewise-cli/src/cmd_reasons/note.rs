//! Adding reasons after the fact: note command.
use super::super::*;

pub(crate) fn run_note(json: bool, repo_paths: &RepoPaths, last: Option<String>, change_id: Option<String>, why: Option<String>) {
    let db_path = repo_paths.data_dir().join("trail.db");
    if !db_path.exists() {
        eprintln!("trail: not initialized; run trail init first");
        process::exit(1);
    }
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("trail: failed to open db: {e}");
            process::exit(1);
        }
    };

    if let Some(why_text) = last {
        note_last(&db, json, why_text);
    } else if let (Some(cid), Some(why_text)) = (change_id, why) {
        note_by_id(&db, json, &cid, why_text);
    } else {
        eprintln!("trail: note requires either <CHANGE_ID> <WHY> or --last <TEXT>");
        process::exit(1);
    }
}

fn note_last(db: &Database, json: bool, why_text: String) {
    if let Err(e) = validate_why(&why_text) {
        eprintln!("trail: {e}");
        process::exit(1);
    }

    let actor = if let Some(a) = Actor::from_env() {
        a
    } else {
        let who = tracewise_proc::who(None, None);
        Actor { kind: who.kind, name: if who.name.is_empty() { None } else { Some(who.name) }, session: who.session }
    };

    let pending = db.list_pending_changes().unwrap_or_default();
    let matching: Vec<_> = pending
        .into_iter()
        .filter(|c| {
            if actor.kind == "agent" {
                // another session of the same agent is another job: its changes are not ours to explain
                c.actor.kind == "agent"
                    && c.actor.name == actor.name
                    && (actor.session.is_none() || c.actor.session.is_none() || c.actor.session == actor.session)
            } else {
                c.actor.kind == "human" && (c.actor.name == actor.name || c.actor.name.is_none() || actor.name.is_none())
            }
        })
        .collect();

    let now_dt = tracewise_core::clock::now();
    let created = now_dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let expires = (now_dt + chrono::Duration::hours(4)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let paths: Vec<String> = matching.iter().map(|c| c.path.clone()).collect();
    let intent_id = ulid::Ulid::new().to_string();
    let intent = Intent {
        id: intent_id.clone(),
        actor: actor.clone(),
        paths,
        why: why_text,
        task: None,
        created,
        expires,
        kind: "note".to_string(),
        ended: 0,
    };
    let _ = db.insert_intent(&intent);
    for c in &matching {
        let _ = db.link_intent_to_change(&c.id, &intent_id);
    }

    if json {
        println!("{}", serde_json::json!({ "noted": matching.len() }));
    } else {
        println!("Noted {} change(s).", matching.len());
    }
}

fn note_by_id(db: &Database, json: bool, cid: &str, why_text: String) {
    if let Err(e) = validate_why(&why_text) {
        eprintln!("trail: {e}");
        process::exit(1);
    }

    let ch = match db.get_change(cid) {
        Ok(Some(c)) => c,
        _ => {
            eprintln!("trail: change not found: {cid}");
            process::exit(1);
        }
    };

    let now_dt = tracewise_core::clock::now();
    let created = now_dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let expires = (now_dt + chrono::Duration::hours(4)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let intent = Intent {
        id: ulid::Ulid::new().to_string(),
        actor: ch.actor,
        paths: vec![ch.path],
        why: why_text,
        task: None,
        created,
        expires,
        kind: "note".to_string(),
        ended: 0,
    };

    let _ = db.insert_intent(&intent);
    let _ = db.link_intent_to_change(cid, &intent.id);

    if json {
        println!("{}", serde_json::json!({ "id": intent.id }));
    } else {
        println!("Added note to change {cid}");
    }
}
