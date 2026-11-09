//! Agent matching table and custom agent configuration parsing.
use super::{get_proc_args, ProcessEntry};
use std::fs;
use std::path::Path;

pub const BUILTIN_SIGNING_IDS: &[(&str, &str)] = &[("com.anthropic.claude-code", "claude")];

pub const BUILTIN_AGENTS: &[(&str, &[&str])] = &[
    ("claude", &["claude"]),
    ("codex", &["codex"]),
    ("gemini", &["gemini"]),
    ("antigravity", &["antigravity", "agy", "Antigravity", "Antigravity Helper*"]),
    ("cursor", &["cursor", "cursor-agent", "Cursor Helper*"]),
    ("aider", &["aider"]),
];

pub fn pattern_matches(pattern: &str, candidate: &str) -> bool {
    let p = pattern.to_ascii_lowercase();
    let c = candidate.to_ascii_lowercase();
    if let Some(prefix) = p.strip_suffix('*') {
        c.starts_with(prefix)
    } else {
        c == p
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomAgentConfig {
    pub name: String,
    pub process: Vec<String>,
}

pub fn parse_agents_toml(content: &str) -> Vec<CustomAgentConfig> {
    let mut agents = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_process: Vec<String> = Vec::new();
    let mut in_agent = false;

    for line in content.lines() {
        let line = if let Some(idx) = line.find('#') { &line[..idx] } else { line }.trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[agent]]" {
            if in_agent {
                if let Some(name) = current_name.take() {
                    agents.push(CustomAgentConfig { name, process: std::mem::take(&mut current_process) });
                }
            }
            in_agent = true;
            current_name = None;
            current_process.clear();
            continue;
        }
        if in_agent {
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                if k == "name" {
                    let unquoted = v.trim_matches('"').trim_matches('\'').trim();
                    current_name = Some(unquoted.to_string());
                } else if k == "process" {
                    let inner = v.trim_matches(|c| c == '[' || c == ']' || c == ' ');
                    for item in inner.split(',') {
                        let item = item.trim().trim_matches('"').trim_matches('\'').trim();
                        if !item.is_empty() {
                            current_process.push(item.to_string());
                        }
                    }
                }
            }
        }
    }
    if in_agent {
        if let Some(name) = current_name.take() {
            agents.push(CustomAgentConfig { name, process: current_process });
        }
    }
    agents
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentTable {
    pub custom: Vec<CustomAgentConfig>,
}

impl AgentTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_from_dir(tracewise_home: &Path) -> Self {
        let path = tracewise_home.join("agents.toml");
        if let Ok(content) = fs::read_to_string(&path) {
            Self::load_from_content(&content)
        } else {
            Self::new()
        }
    }

    pub fn load_from_content(content: &str) -> Self {
        Self { custom: parse_agents_toml(content) }
    }

    pub fn load_default() -> Self {
        Self::load_from_dir(&super::tracewise_home_dir())
    }

    pub fn match_signing_id(&self, id: &str) -> Option<String> {
        let id = id.trim();
        if id.is_empty() {
            return None;
        }
        for agent in &self.custom {
            if agent.process.iter().any(|p| p == id) {
                return Some(agent.name.clone());
            }
        }
        BUILTIN_SIGNING_IDS.iter().find(|(sig, _)| *sig == id).map(|(_, name)| (*name).to_string())
    }

    pub fn match_candidate(&self, candidate: &str) -> Option<String> {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            return None;
        }

        for agent in &self.custom {
            for pattern in &agent.process {
                if pattern_matches(pattern, trimmed) {
                    return Some(agent.name.clone());
                }
            }
        }

        for (agent_name, patterns) in BUILTIN_AGENTS {
            for pattern in *patterns {
                if pattern_matches(pattern, trimmed) {
                    return Some((*agent_name).to_string());
                }
            }
        }

        None
    }

    pub fn match_process(&self, entry: &ProcessEntry) -> Option<String> {
        let exe_lower = entry.name.to_ascii_lowercase();
        let is_runtime_wrapper = matches!(exe_lower.as_str(), "node" | "bun" | "deno" | "python" | "python3");

        if is_runtime_wrapper {
            if let Some(args) = get_proc_args(entry.pid) {
                if let Some(argv1) = args.get(1) {
                    if let Some(file_name) = Path::new(argv1).file_name().and_then(|f| f.to_str()) {
                        if let Some(agent) = self.match_candidate(file_name) {
                            return Some(agent);
                        }
                        if let Some(stem) = Path::new(argv1).file_stem().and_then(|f| f.to_str()) {
                            if let Some(agent) = self.match_candidate(stem) {
                                return Some(agent);
                            }
                        }
                    }
                }
            }
        }

        self.match_candidate(&entry.name)
    }
}

pub fn agent_for_writer(table: &AgentTable, exe: &str, signing_id: Option<&str>, pid: u32, chain: &[ProcessEntry]) -> Option<String> {
    if let Some(agent) = signing_id.and_then(|s| table.match_signing_id(s)) {
        return Some(agent);
    }
    let name = Path::new(exe).file_name().and_then(|n| n.to_str()).unwrap_or(exe).to_string();
    let me = ProcessEntry { pid, ppid: 0, name, path: exe.to_string() };
    std::iter::once(&me).chain(chain.iter()).find_map(|e| table.match_process(e))
}
