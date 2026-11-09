//! Actor resolution, confidence deduction, and lock conflict checking during scans.
use super::ScanOptions;
use crate::db::Database;
use crate::model::{glob_match, Actor};

pub(crate) fn resolve_actor_and_confidence(options: &ScanOptions, path: &str) -> (Actor, String) {
    if let Some(predicted) = &options.predicted_paths {
        let actor = options.actor.clone().or_else(Actor::from_env).unwrap_or_else(Actor::unknown);
        let confidence = if predicted.contains(path) { "exact".to_string() } else { "likely".to_string() };
        return (actor, confidence);
    }

    let actor = options.actor.clone().or_else(Actor::from_env).unwrap_or_else(Actor::unknown);

    let confidence = options.confidence.clone().unwrap_or_else(|| {
        if options.actor.is_some() || Actor::from_env().is_some() {
            "exact".to_string()
        } else {
            "unknown".to_string()
        }
    });

    (actor, confidence)
}

pub(crate) fn human_actor() -> Actor {
    let name = std::env::var("TRACEWISE_HUMAN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("USER").ok().filter(|s| !s.trim().is_empty()));
    Actor { kind: "human".to_string(), name, session: None }
}

pub(crate) fn resolve_actor_candidates_confidence(
    db: &Database,
    options: &ScanOptions,
    path: &str,
    is_modify: bool,
) -> (Actor, Vec<Actor>, String, Option<String>) {
    // The strict sensor saw the process that wrote this file: nothing the windows say can be better evidence.
    if options.source == "sensor" {
        if let Some(actor) = &options.actor {
            return (actor.clone(), Vec::new(), "exact".to_string(), None);
        }
    }
    let now_str = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let start_str = options.window_start.as_deref().unwrap_or(&now_str);
    let overlapping = db.get_overlapping_windows(start_str, &now_str).unwrap_or_default();

    let mut distinct_actors: Vec<Actor> = Vec::new();
    // Background jobs are the weakest evidence: they only decide when nothing else applies.
    let mut background: Vec<Actor> = Vec::new();
    let mut long_shell: Vec<Actor> = Vec::new();
    for w in overlapping.iter().filter(|w| window_is_live(db, w)) {
        if w.kind == "agent_background" {
            if !background.iter().any(|a| a.is_same(&w.actor)) {
                background.push(w.actor.clone());
            }
            continue;
        }
        let applies = if w.kind == "agent_edit" { w.paths.iter().any(|p| p == path || glob_match(p, path)) } else { true };
        // a command that has run for minutes (a dev server, a watcher) says little about who wrote this file
        if applies && w.kind == "shell" && shell_window_is_long_running(w) {
            if !long_shell.iter().any(|a| a.is_same(&w.actor)) {
                long_shell.push(w.actor.clone());
            }
            continue;
        }
        if applies && !distinct_actors.iter().any(|a| a.is_same(&w.actor)) {
            distinct_actors.push(w.actor.clone());
        } else if applies {
            // the same actor seen twice: keep the entry that knows more (a person's name, an agent's session)
            if let Some(known) = distinct_actors.iter_mut().find(|a| {
                a.is_same(&w.actor)
                    && ((a.name.is_none() && w.actor.name.is_some())
                        || (a.kind == "agent" && a.session.is_none() && w.actor.session.is_some()))
            }) {
                *known = w.actor.clone();
            }
        }
    }
    if let Some(ref opt_actor) = options.actor {
        if !distinct_actors.iter().any(|a| a.is_same(opt_actor)) {
            distinct_actors.push(opt_actor.clone());
        }
    }

    // Long-running shell commands only count when no agent is a candidate: an agent's own edit or command window wins.
    if !distinct_actors.iter().any(|a| a.kind == "agent") {
        for a in long_shell {
            if !distinct_actors.iter().any(|d| d.is_same(&a)) {
                distinct_actors.push(a);
            }
        }
    }

    if distinct_actors.len() > 1 {
        return (Actor { kind: "ambiguous".to_string(), name: None, session: None }, distinct_actors, "unknown".to_string(), None);
    }

    if distinct_actors.is_empty() && options.actor.is_none() && !background.is_empty() {
        let source = Some("background".to_string());
        if background.len() > 1 {
            let ambiguous = Actor { kind: "ambiguous".to_string(), name: None, session: None };
            return (ambiguous, background, "unknown".to_string(), source);
        }
        return (background.remove(0), Vec::new(), "likely".to_string(), source);
    }

    let (mut actor, mut conf) = resolve_actor_and_confidence(options, path);
    if actor.kind == "human" && actor.name.is_none() {
        if let Some(named) = distinct_actors.iter().find(|a| a.kind == "human" && a.name.is_some()) {
            actor = named.clone();
        }
    }
    if is_modify
        && actor.kind == "unknown"
        && options.actor.is_none()
        && distinct_actors.is_empty()
        && background.is_empty()
        && !overlapping.iter().any(|w| (w.kind.starts_with("agent") || w.actor.kind == "agent") && window_is_live(db, w))
    {
        actor = human_actor();
        conf = "likely".to_string();
    }
    (actor, Vec::new(), conf, None)
}

/// A shell command open for more than this is a long-running one (a dev server, a build watcher).
const LONG_SHELL_SECS: i64 = 120;

fn shell_window_is_long_running(w: &crate::model::Window) -> bool {
    chrono::DateTime::parse_from_rfc3339(&w.start)
        .map(|t| crate::clock::now().signed_duration_since(t).num_seconds() > LONG_SHELL_SECS)
        .unwrap_or(false)
}

/// A shell that exited (closed tab, `exit`, crash) never sends its closing hook, so its window would stay open
/// and make every later change ambiguous. Such windows are closed here; a window left open for hours (2 for an agent, 24 otherwise) is ignored.
fn window_is_live(db: &Database, w: &crate::model::Window) -> bool {
    if w.kind == "agent_background" {
        // a background window is written with the time it expires as `end`; a stop moves `end` to now
        let now = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        return w.end.as_deref().is_some_and(|e| e > now.as_str());
    }
    let shell_pid = w.id.strip_prefix("shell_proc_").or_else(|| w.id.strip_prefix("shell_")).and_then(|rest| rest.parse::<i32>().ok());
    let dead_shell = w.kind == "shell" && shell_pid.is_some_and(|pid| !pid_alive(pid));
    // an agent whose tool call never reported back (crash) leaves its window open: give those 2 hours
    // a tool call of an agent is over within minutes; one that never reported back failed or was interrupted
    let max_age = match w.kind.as_str() {
        "agent_edit" | "agent_command" => chrono::Duration::minutes(15),
        k if k.starts_with("agent") => chrono::Duration::hours(2),
        _ => chrono::Duration::hours(24),
    };
    let too_old = w.end.is_none()
        && chrono::DateTime::parse_from_rfc3339(&w.start).map(|t| crate::clock::now().signed_duration_since(t) > max_age).unwrap_or(false);
    if dead_shell || too_old {
        let now = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let _ = db.close_window_by_id(&w.id, &now);
        return false;
    }
    true
}

fn pid_alive(pid: i32) -> bool {
    tracewise_platform::signal::is_process_running(pid)
}

pub(crate) fn check_lock_conflict(db: &Database, path: &str, current_actor: &Actor) -> Option<serde_json::Value> {
    if let Ok(Some(lock)) = db.get_lock_for_path(path) {
        let is_conflict = match (&lock.holder.session, &current_actor.session) {
            (Some(s1), Some(s2)) => s1 != s2,
            (Some(_), None) => true,
            _ => lock.holder.name != current_actor.name,
        };
        if is_conflict {
            return serde_json::to_value(&lock.holder).ok();
        }
    }
    None
}
