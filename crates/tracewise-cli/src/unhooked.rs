//! Unhooked agent writes: a change the daemon saw while an agent was working here, that no hook explained.
//!
//! The attribution of a change is not touched (it stays `unknown`, or the person, as the evidence says). After the daemon records a batch
//! of changes it saw only through file events, this looks for agents whose hooks fired in the last `RECENT_SECS` seconds. If there is one
//! and a change in the batch got no agent, one command of kind `unhooked` is recorded, naming the agent and the files. The usual causes
//! are a tool that skips the hooks (a full path such as `/bin/rm`, a script, an editor plugin), a formatter that runs on save after the
//! edit window closed, or a person saving a file while an agent happens to be working: the note says "possibly" for that reason.
use chrono::Duration;
use std::fs;
use std::path::Path;
use tracewise_core::db::Database;
use tracewise_core::model::{Actor, Change, CommandRecord};

/// An agent counts as working here when one of its hook windows overlaps the last this many seconds.
const RECENT_SECS: i64 = 30;
/// At most one note per repository in this many seconds, so a long burst of writes is one note and not hundreds.
const NOTE_EVERY_SECS: i64 = 30;
const SHOWN_PATHS: usize = 3;

/// The agents (each once) whose hooks fired within the last `RECENT_SECS` seconds.
pub(crate) fn recent_agents(db: &Database) -> Vec<Actor> {
    let now = tracewise_core::clock::now();
    let since = (now - Duration::seconds(RECENT_SECS)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let until = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut out: Vec<Actor> = Vec::new();
    for w in db.get_overlapping_windows(&since, &until).unwrap_or_default() {
        // a window that never closed is not evidence of recent work unless it began recently
        let recent = w.end.is_some() || w.start >= since;
        if w.actor.kind == "agent" && recent && !out.iter().any(|a| a.is_same(&w.actor)) {
            out.push(w.actor);
        }
    }
    out
}

/// Changes the daemon saw only through file events and that nobody explained: no agent, and no hook, shell or git window.
pub(crate) fn unexplained(changes: &[Change]) -> Vec<&Change> {
    changes
        .iter()
        .filter(|c| c.source == "watch" && c.op != "baseline")
        .filter(|c| c.actor.kind == "unknown" || (c.actor.kind == "human" && c.confidence == "likely"))
        .collect()
}

/// The sentence of the note.
pub(crate) fn text(agents: &[Actor], paths: &[&str]) -> String {
    let who = agents.iter().map(|a| a.name.clone().unwrap_or_else(|| "an agent".to_string())).collect::<Vec<_>>().join(" and ");
    let shown: Vec<&str> = paths.iter().copied().take(SHOWN_PATHS).collect();
    let more = if paths.len() > SHOWN_PATHS { format!(" and {} more", paths.len() - SHOWN_PATHS) } else { String::new() };
    let n = paths.len();
    format!(
        "{n} change{} possibly by {who} with no hook explaining {}: {}{more}",
        if n == 1 { "" } else { "s" },
        if n == 1 { "it" } else { "them" },
        shown.join(", ")
    )
}

fn recently_noted(data_dir: &Path) -> bool {
    let Ok(t) = fs::read_to_string(data_dir.join("unhooked.last")) else { return false };
    chrono::DateTime::parse_from_rfc3339(t.trim())
        .map(|at| (tracewise_core::clock::now() - at.with_timezone(&chrono::Utc)).num_seconds() < NOTE_EVERY_SECS)
        .unwrap_or(false)
}

/// Called after the daemon records a batch it saw only through file events. Records at most one `unhooked` note. Fails open.
pub(crate) fn note(db: &Database, data_dir: &Path, ids: &[String]) {
    if ids.is_empty() || recently_noted(data_dir) {
        return;
    }
    let agents = recent_agents(db);
    if agents.is_empty() {
        return;
    }
    let changes: Vec<Change> = ids.iter().filter_map(|id| db.get_change(id).ok().flatten()).collect();
    let lost = unexplained(&changes);
    if lost.is_empty() {
        return;
    }
    let paths: Vec<&str> = lost.iter().map(|c| c.path.as_str()).collect();
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let actor = if agents.len() == 1 { agents[0].clone() } else { Actor { kind: "ambiguous".to_string(), name: None, session: None } };
    let rec = CommandRecord {
        id: ulid::Ulid::new().to_string(),
        kind: "unhooked".to_string(),
        text: text(&agents, &paths),
        cwd: None,
        actor,
        start: now.clone(),
        end: now.clone(),
        exit_code: 0,
        changes: lost.iter().map(|c| c.id.clone()).collect(),
    };
    // not linked to the changes: a linked command would replace "Needs a reason" in the Timeline
    if db.insert_command(&rec).is_ok() {
        let _ = tracewise_platform::private::write(data_dir.join("unhooked.last"), now);
    }
}

#[cfg(test)]
#[path = "test_unhooked.rs"]
mod tests;
