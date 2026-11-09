//! Adapters as files: `<name>.toml` in `<repo>/.tracewise/adapters/` (wins) or `$TRACEWISE_HOME/adapters/` says how an
//! agent's hook payload becomes Tracewise's events and what to answer. The file is data: nothing in it is ever run.
use super::{execute_hook, HookInput, HookOutput};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const BUILTIN: [&str; 5] = ["claude", "antigravity", "codex", "cursor", "gemini"];
const EVENTS: [&str; 7] = ["pre-edit", "post-edit", "pre-command", "post-command", "stop", "session-end", "prompt"];
const DEFAULT_ALLOW: &str = "{}";
const DEFAULT_DENY: &str = r#"{"decision":"deny","reason":"{reason}"}"#;

#[derive(Debug, Default, Clone)]
pub struct Adapter {
    pub name: String,
    session: String,
    cwd: String,
    event_ptr: String,
    event_map: HashMap<String, String>,
    edit_tools_ptr: String,
    edit_tools: Vec<String>,
    edit_path: String,
    cmd_tools_ptr: String,
    cmd_tools: Vec<String>,
    cmd_text: String,
    allow: String,
    deny: String,
    /// `(file, JSON content)` from `[install]`.
    pub install: Option<(String, String)>,
}

/// What one payload becomes.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Translation {
    pub event: Option<String>,
    pub session: String,
    pub paths: Vec<String>,
    pub command: Option<String>,
    pub tool: Option<String>,
    pub cwd: Option<String>,
}

pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn text(t: &toml::Table, section: &str, key: &str) -> Option<String> {
    t.get(section)?.get(key)?.as_str().map(String::from)
}

fn list(t: &toml::Table, section: &str, key: &str) -> Vec<String> {
    t.get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

impl Adapter {
    pub fn parse(name: &str, source: &str) -> Result<Adapter, String> {
        let t: toml::Table = toml::from_str(source).map_err(|e| e.to_string().replace('\n', " "))?;
        let map = t
            .get("events")
            .and_then(|e| e.get("map"))
            .and_then(|m| m.as_table())
            .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect())
            .unwrap_or_default();
        let ptr = |section: &str, key: &str, default: &str| text(&t, section, key).unwrap_or_else(|| default.to_string());
        let install = text(&t, "install", "file").zip(text(&t, "install", "content"));
        if let Some((_, content)) = &install {
            serde_json::from_str::<Value>(content).map_err(|e| format!("[install] content is not JSON: {e}"))?;
        }
        Ok(Adapter {
            name: name.to_string(),
            session: ptr("agent", "session", "/session"),
            cwd: ptr("agent", "cwd", "/cwd"),
            event_ptr: ptr("events", "pointer", "/event"),
            event_map: map,
            edit_tools_ptr: ptr("edit", "tools_pointer", "/tool"),
            edit_tools: list(&t, "edit", "tools"),
            edit_path: ptr("edit", "path", "/path"),
            cmd_tools_ptr: ptr("command", "tools_pointer", "/tool"),
            cmd_tools: list(&t, "command", "tools"),
            cmd_text: ptr("command", "text", "/command"),
            allow: ptr("reply", "allow", DEFAULT_ALLOW),
            deny: ptr("reply", "deny", DEFAULT_DENY),
            install,
        })
    }

    /// The event, session, paths and command a payload stands for. `cli_event` is used when the payload names no event.
    pub fn translate(&self, cli_event: &str, payload: &Value) -> Translation {
        let get = |ptr: &str| payload.pointer(ptr).and_then(|v| v.as_str()).map(String::from);
        let raw = get(&self.event_ptr).unwrap_or_else(|| cli_event.to_string());
        let mapped = self.event_map.get(&raw).map(String::as_str).unwrap_or("");
        let edit_tool = get(&self.edit_tools_ptr).filter(|t| self.edit_tools.contains(t));
        let cmd_tool = get(&self.cmd_tools_ptr).filter(|t| self.cmd_tools.contains(t));
        let event = match mapped {
            "pre-tool" | "post-tool" => {
                let phase = mapped.trim_end_matches("-tool");
                match (&edit_tool, &cmd_tool) {
                    (Some(_), _) => Some(format!("{phase}-edit")),
                    (_, Some(_)) => Some(format!("{phase}-command")),
                    _ => None,
                }
            }
            other => EVENTS.iter().find(|e| **e == other).map(|e| e.to_string()),
        };
        let paths = match payload.pointer(&self.edit_path) {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
            _ => Vec::new(),
        };
        let is_edit = event.as_deref().is_some_and(|e| e.ends_with("-edit"));
        let is_cmd = event.as_deref().is_some_and(|e| e.ends_with("-command"));
        Translation {
            event,
            session: get(&self.session).unwrap_or_else(|| "default".to_string()),
            paths: if is_edit { paths } else { Vec::new() },
            command: if is_cmd { get(&self.cmd_text) } else { None },
            tool: edit_tool.or(cmd_tool),
            cwd: get(&self.cwd).filter(|c| !c.is_empty()),
        }
    }

    fn reply(&self, deny: Option<&str>) -> String {
        match deny {
            None => self.allow.clone(),
            Some(reason) => {
                let quoted = serde_json::to_string(reason).unwrap_or_default();
                self.deny.replace("{reason}", quoted.trim_matches('"'))
            }
        }
    }
}

fn adapter_dirs(work_tree: Option<&Path>) -> Vec<(&'static str, PathBuf)> {
    let mut dirs = Vec::new();
    if let Some(wt) = work_tree {
        dirs.push(("project", wt.join(".tracewise").join("adapters")));
    }
    dirs.push(("user", tracewise_core::get_tracewise_home().join("adapters")));
    dirs
}

/// The adapter file for `name`: the project's wins over the user's. `Ok(None)` when there is none; `Err` names a broken file.
pub fn find(name: &str, work_tree: Option<&Path>) -> Result<Option<(Adapter, &'static str)>, String> {
    if !is_valid_name(name) || BUILTIN.contains(&name) {
        return Ok(None);
    }
    for (source, dir) in adapter_dirs(work_tree) {
        let file = dir.join(format!("{name}.toml"));
        if let Ok(body) = std::fs::read_to_string(&file) {
            return Adapter::parse(name, &body).map(|a| Some((a, source))).map_err(|e| format!("adapter {name} ({}): {e}", file.display()));
        }
    }
    Ok(None)
}

/// Every adapter file as `(name, source, error)`; the project's file hides the user's of the same name.
pub fn list_files(work_tree: Option<&Path>) -> Vec<(String, &'static str, Option<String>)> {
    let mut seen = std::collections::BTreeMap::new();
    for (source, dir) in adapter_dirs(work_tree) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let Some(name) =
                path.file_stem().and_then(|s| s.to_str()).filter(|n| path.extension().is_some_and(|e| e == "toml") && is_valid_name(n))
            else {
                continue;
            };
            let err = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|b| Adapter::parse(name, &b).map(|_| ())).err();
            seen.entry(name.to_string()).or_insert((source, err.map(|e| format!("{}: {e}", path.display()))));
        }
    }
    seen.into_iter().map(|(n, (s, e))| (n, s, e)).collect()
}

/// Answers a hook for an agent that is described by a file. Fails open: any problem allows.
pub fn handle_file_hook(adapter: &Adapter, cli_event: &str, stdin: &str) {
    let payload: Value = serde_json::from_str(stdin).unwrap_or(Value::Null);
    let tr = adapter.translate(cli_event, &payload);
    let Some(event) = tr.event.clone() else {
        println!("{}", adapter.reply(None));
        return;
    };
    let input =
        HookInput { agent: Some(adapter.name.clone()), session: Some(tr.session), paths: tr.paths, command: tr.command, cwd: tr.cwd };
    let res = execute_hook(&event, input).unwrap_or(HookOutput { decision: "allow", reason: None, notice: None });
    let can_refuse = matches!(event.as_str(), "pre-edit" | "pre-command" | "stop");
    if can_refuse && matches!(res.decision, "deny" | "block") {
        println!("{}", adapter.reply(Some(&res.reason.unwrap_or_default())));
    } else {
        println!("{}", adapter.reply(None));
    }
}

#[cfg(test)]
#[path = "test_adapter.rs"]
mod tests;
