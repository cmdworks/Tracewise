//! Self-healing: the daemon's sweep puts back agent hooks that were removed, and records a stretch in which Tracewise was not running.
//!
//! `tracewise agent install <agent>` writes the agent's name into `hooks-wanted.json` in the data folder and `agent uninstall` takes it out, so
//! a person who removes the hooks on purpose is never fought: only hooks that are *wanted* and missing are restored. The sweep also writes
//! `heartbeat.json` (the daemon's pid and the time). When the pid changed and the last beat is older than `GAP_SECS`, the daemon was
//! stopped or killed and restarted (launchd's `KeepAlive`), and the stretch is recorded; the catch-up scan at the restart has already
//! recorded what changed meanwhile. The same pid with an old beat is a sleeping Mac and records nothing.
use crate::cmd_agents::{install_agent_quiet, missing_events, AGENT_CONFIGS};
use crate::RepoPaths;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracewise_core::db::Database;
use tracewise_core::model::{Actor, CommandRecord};

/// A restart is recorded when the previous beat is older than this (the sweep runs every 30 s).
const GAP_SECS: i64 = 90;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub(crate) struct Heartbeat {
    pub pid: u32,
    pub at: String,
}

fn wanted_path(repo: &RepoPaths) -> PathBuf {
    repo.data_dir().join("hooks-wanted.json")
}

/// The agents whose hooks this repository wants (written by `agent install`).
pub(crate) fn wanted(repo: &RepoPaths) -> Vec<String> {
    fs::read_to_string(wanted_path(repo)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Adds or removes `name`; idempotent. Only built-in agents with a config file are listed.
pub(crate) fn set_wanted(repo: &RepoPaths, name: &str, on: bool) {
    if !AGENT_CONFIGS.iter().any(|(a, _)| *a == name) {
        return;
    }
    let mut list = wanted(repo);
    let had = list.iter().any(|a| a == name);
    if on && !had {
        list.push(name.to_string());
    } else if !on && had {
        list.retain(|a| a != name);
    } else {
        return;
    }
    list.sort();
    if repo.data_dir().is_dir() {
        let _ = tracewise_platform::private::write(wanted_path(repo), serde_json::to_string(&list).unwrap_or_default());
    }
}

fn hooks_missing(repo: &RepoPaths, name: &str) -> bool {
    let Some((_, cfg)) = AGENT_CONFIGS.iter().find(|(a, _)| *a == name) else { return false };
    let text = fs::read_to_string(repo.work_tree.join(cfg)).unwrap_or_default();
    !missing_events(name, &text).is_empty()
}

fn note(db: &Database, kind: &str, text: String, start: &str) {
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let rec = CommandRecord {
        id: ulid::Ulid::new().to_string(),
        kind: kind.to_string(),
        text,
        cwd: None,
        actor: Actor { kind: "script".to_string(), name: Some("tracewise".to_string()), session: None },
        start: start.to_string(),
        end: now,
        exit_code: 0,
        changes: Vec::new(),
    };
    let _ = db.insert_command(&rec);
}

/// Puts back the wanted hooks that are gone, one record each. Returns the agents it restored.
pub(crate) fn repair_hooks(repo: &RepoPaths, db: &Database) -> Vec<String> {
    let mut fixed = Vec::new();
    for name in wanted(repo) {
        if hooks_missing(repo, &name) && install_agent_quiet(repo, &name).is_ok() {
            let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            note(db, "repair", format!("{name}'s Tracewise hooks were missing from its config; Tracewise put them back"), &now);
            fixed.push(name);
        }
    }
    fixed
}

/// The stretch to record, when `prev` is the beat of a daemon other than `pid` and is older than `min_secs`: (from, to).
pub(crate) fn gap(prev: &Heartbeat, pid: u32, now: DateTime<Utc>, min_secs: i64) -> Option<(String, String)> {
    if prev.pid == pid {
        return None;
    }
    let at = DateTime::parse_from_rfc3339(&prev.at).ok()?.with_timezone(&Utc);
    ((now - at).num_seconds() > min_secs).then(|| (prev.at.clone(), now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)))
}

/// One beat: records a gap when the daemon was replaced, then writes the new heartbeat.
pub(crate) fn heartbeat(repo: &RepoPaths, db: &Database, pid: u32) {
    let path = repo.data_dir().join("heartbeat.json");
    let now = tracewise_core::clock::now();
    if let Some(prev) = fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Heartbeat>(&t).ok()) {
        if let Some((from, to)) = gap(&prev, pid, now, GAP_SECS) {
            note(
                db,
                "gap",
                format!("Tracewise was not running from {from} to {to}; what changed in between was picked up by the catch-up scan"),
                &from,
            );
        }
    }
    let beat = Heartbeat { pid, at: now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true) };
    let _ = tracewise_platform::private::write(&path, serde_json::to_string(&beat).unwrap_or_default());
}

/// Called by `tracewise daemon sweep` for each repository (never prints: the daemon reads the sweep's output). Fails open.
pub(crate) fn on_sweep(repo: &RepoPaths, db: &Database) {
    if let Some(pid) =
        fs::read_to_string(tracewise_core::get_tracewise_home().join("daemon.pid")).ok().and_then(|t| t.trim().parse::<u32>().ok())
    {
        heartbeat(repo, db, pid);
    }
    repair_hooks(repo, db);
}

#[cfg(test)]
#[path = "test_self_heal.rs"]
mod tests;
