//! Process information and parent chain inspection for Tracewise.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

pub mod sys;
pub mod table;
#[cfg(test)]
mod tests;

pub use sys::{
    chain, exe, get_ppid, get_proc_args, get_proc_name, get_proc_path, get_process_entry, is_alive, name, parent, parent_pid, process_chain,
};
pub use table::{agent_for_writer, parse_agents_toml, AgentTable, CustomAgentConfig, BUILTIN_AGENTS, BUILTIN_SIGNING_IDS};
pub use tracewise_platform::process::ProcessEntry;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessMatch {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcWhoResult {
    pub kind: String,
    pub name: String,
    pub session: Option<String>,
    pub source: String,
    pub matched: Option<ProcessMatch>,
}

pub fn parse_actor_str(val: &str) -> Option<(String, String, Option<String>)> {
    let parts: Vec<&str> = val.split(':').collect();
    if parts.len() >= 2 {
        let kind = parts[0].to_string();
        let name = parts[1].to_string();
        let session = if parts.len() >= 3 && !parts[2].is_empty() { Some(parts[2].to_string()) } else { None };
        Some((kind, name, session))
    } else {
        None
    }
}

fn actor_env_non_human() -> Option<ProcWhoResult> {
    let trimmed = std::env::var("TRAIL_ACTOR").ok()?;
    let (kind, name, session) = parse_actor_str(trimmed.trim()).filter(|(k, _, _)| k != "human")?;
    Some(ProcWhoResult { kind, name, session, source: "env".to_string(), matched: None })
}

fn fallback_human() -> ProcWhoResult {
    if let Ok(actor_val) = std::env::var("TRAIL_ACTOR") {
        if let Some((kind, name, session)) = parse_actor_str(actor_val.trim()).filter(|(k, _, _)| k == "human") {
            return ProcWhoResult { kind, name, session, source: "env".to_string(), matched: None };
        }
    }
    let human_name = std::env::var("TRAIL_HUMAN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("USER").ok().filter(|s| !s.trim().is_empty()))
        .unwrap_or_else(|| "unknown".to_string());
    ProcWhoResult { kind: "human".to_string(), name: human_name, session: None, source: "default".to_string(), matched: None }
}

/// Determine who is acting following the priority:
/// 1. TRAIL_ACTOR (source "env")
/// 2. Nearest process in the chain matching the agent table (source "process")
/// 3. Human default from TRAIL_HUMAN or USER (source "default")
pub fn who(pid: Option<u32>, table: Option<&AgentTable>) -> ProcWhoResult {
    if let Some(res) = actor_env_non_human() {
        return res;
    }
    let default_table;
    let agent_table = match table {
        Some(t) => t,
        None => {
            default_table = AgentTable::load_default();
            &default_table
        }
    };

    let mut curr = pid.unwrap_or_else(parent_pid);
    let mut visited = HashSet::new();
    while !visited.contains(&curr) && visited.len() < 128 {
        visited.insert(curr);
        let Some(entry) = get_process_entry(curr) else { break };
        let next_pid = entry.ppid;
        let is_root = curr == 0 || curr == 1;
        if let Some(agent_name) = agent_table.match_process(&entry) {
            return ProcWhoResult {
                kind: "agent".to_string(),
                name: agent_name,
                session: None,
                source: "process".to_string(),
                matched: Some(ProcessMatch { pid: curr, name: entry.name }),
            };
        }
        if is_root || next_pid == curr {
            break;
        }
        curr = next_pid;
    }
    marker_agent().unwrap_or_else(fallback_human)
}

/// Same priority as `who`, evaluated on a known process chain (no live process calls).
pub fn who_with_chain(chain: &[ProcessEntry], table: Option<&AgentTable>) -> ProcWhoResult {
    if let Some(res) = actor_env_non_human() {
        return res;
    }
    let default_table;
    let agent_table = match table {
        Some(t) => t,
        None => {
            default_table = AgentTable::load_default();
            &default_table
        }
    };
    for entry in chain {
        if let Some(agent_name) = agent_table.match_process(entry) {
            return ProcWhoResult {
                kind: "agent".to_string(),
                name: agent_name,
                session: None,
                source: "process".to_string(),
                matched: Some(ProcessMatch { pid: entry.pid, name: entry.name.clone() }),
            };
        }
    }
    marker_agent().unwrap_or_else(fallback_human)
}

/// An agent that marks its commands' environment (Claude Code, Antigravity), whatever the process chain shows.
fn marker_agent() -> Option<ProcWhoResult> {
    let set = |k: &str| std::env::var(k).ok().as_deref() == Some("1");
    let (name, session_var) = if set("CLAUDECODE") {
        ("claude", "CLAUDE_CODE_SESSION_ID")
    } else if set("ANTIGRAVITY_AGENT") {
        ("antigravity", "ANTIGRAVITY_CONVERSATION_ID")
    } else {
        return None;
    };
    Some(ProcWhoResult {
        kind: "agent".to_string(),
        name: name.to_string(),
        session: std::env::var(session_var).ok().filter(|s| !s.is_empty()),
        source: "env".to_string(),
        matched: None,
    })
}

/// Resolve `TRAIL_HOME` directory (~/.trail by default).
pub fn trail_home_dir() -> PathBuf {
    if let Ok(val) = std::env::var("TRAIL_HOME") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".trail")
    } else {
        PathBuf::from(".trail")
    }
}
