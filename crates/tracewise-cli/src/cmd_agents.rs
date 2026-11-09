use crate::RepoPaths;
use serde::Serialize;
use serde_json::Value;
use std::fs;

pub const AGENT_CONFIGS: [(&str, &str); 5] = [
    ("claude", ".claude/settings.json"),
    ("codex", ".codex/hooks.json"),
    ("gemini", ".gemini/settings.json"),
    ("antigravity", ".agents/hooks.json"),
    ("cursor", ".cursor/hooks.json"),
];

const TRAIL_BLOCK_START: &str = "<!-- trail:start -->";
const TRAIL_BLOCK_END: &str = "<!-- trail:end -->";

const AGENTS_MD_BLOCK: &str = r#"<!-- trail:start -->
## Trail Rules
Always record reasons for file modifications:
- Declare intent before editing: `trail intent add --why "<why>" <files...>`
- Run commands with a reason: `trail run --why "<why>" -- <command>`
- Review pending changes: `trail pending`
- Attach a reason to your pending changes: `trail note --last "<why>"` (or `trail note <change id> "<why>"`)
<!-- trail:end -->"#;

const CLAUDE_SKILL_MD: &str = r#"# Trail Integration

Trail coordinates file locks and audit logs across agents and humans.

## Rules
- Before editing files, declare your reason: `trail intent add --why "<why>" <files...>`
- When running shell commands that modify files: `trail run --why "<why>" -- <command>`
- Review pending unrecorded changes: `trail pending`
- Add a reason to your pending changes after the fact: `trail note --last "<why>"` (or `trail note <change id> "<why>"`)
"#;

/// Permission rules Claude Code enforces on its own, next to Tracewise's hook refusal: an agent may not edit Tracewise's data or install.
const CLAUDE_DENY: [&str; 4] = ["Edit(**/.git/trail/**)", "Write(**/.git/trail/**)", "Edit(~/.trail/**)", "Write(~/.trail/**)"];

const HOOK_EVENTS: [&str; 4] = ["PreToolUse", "PostToolUse", "Stop", "SessionEnd"];

/// The hook events `install` writes for `agent` (claude also has the prompt and failed-call hooks; antigravity has no SessionEnd).
fn expected_events(agent: &str) -> Vec<&'static str> {
    let base = if agent == "antigravity" { &HOOK_EVENTS[..3] } else { &HOOK_EVENTS[..] };
    base.iter().copied().chain((agent == "claude").then_some(["UserPromptSubmit", "PostToolUseFailure"]).into_iter().flatten()).collect()
}

/// Events `install` would write that the config text does not have: that agent's edits are captured less than "hooks: yes" says.
pub fn missing_events(agent: &str, config_text: &str) -> Vec<&'static str> {
    expected_events(agent).into_iter().filter(|e| !config_text.contains(&format!("trail hook {e} --from {agent}"))).collect()
}

#[derive(Serialize)]
pub struct AgentInstallResult {
    pub agent: String,
    pub files: Vec<String>,
}

#[derive(Serialize)]
pub struct AgentUninstallResult {
    pub agent: String,
    pub uninstalled: bool,
}

#[derive(Serialize)]
pub struct AgentStatusEntry {
    pub name: String,
    pub hooks: bool,
    pub skill: bool,
    pub shims: bool,
    pub shell: bool,
    /// Hook events the config lacks (empty when complete or when there are no hooks at all).
    pub missing_events: Vec<&'static str>,
}

/// Adds the missing `CLAUDE_DENY` rules to `permissions.deny`, keeping every rule the person already has. True when it added any.
fn add_claude_deny_rules(obj: &mut serde_json::Map<String, Value>) -> bool {
    let perms = obj.entry("permissions").or_insert_with(|| Value::Object(serde_json::Map::new()));
    let Some(perms) = perms.as_object_mut() else { return false };
    let deny = perms.entry("deny").or_insert_with(|| Value::Array(Vec::new()));
    let Some(deny) = deny.as_array_mut() else { return false };
    let mut added = false;
    for rule in CLAUDE_DENY {
        if !deny.iter().any(|d| d.as_str() == Some(rule)) {
            deny.push(Value::String(rule.to_string()));
            added = true;
        }
    }
    added
}

fn install_agent_hooks(repo_paths: &RepoPaths, name: &str, agent_cfg: &str) -> Result<String, Box<dyn std::error::Error>> {
    let cfg_path = repo_paths.work_tree.join(agent_cfg);
    if let Some(parent) = cfg_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut val: Value = if cfg_path.exists() {
        let text = fs::read_to_string(&cfg_path).unwrap_or_default();
        match serde_json::from_str(&text) {
            Ok(v) => v,
            // never overwrite what we cannot read: it may be the user's only copy of their settings (M16)
            Err(e) if !text.trim().is_empty() => {
                return Err(
                    format!("{agent_cfg} is not valid JSON ({e}); fix or remove it, then run `trail agent install {name}` again").into()
                )
            }
            Err(_) => Value::Object(serde_json::Map::new()),
        }
    } else {
        Value::Object(serde_json::Map::new())
    };

    if !val.is_object() {
        val = Value::Object(serde_json::Map::new());
    }

    let obj = val.as_object_mut().unwrap();
    let mut changed_hooks = false;
    if name == "antigravity" {
        // Antigravity: a named hook; tool events take {matcher, hooks:[..]} groups, Stop takes command entries.
        let cmd = |e: &str| format!("trail hook {e} --from antigravity");
        let group = |e: &str| serde_json::json!([{"matcher": "*", "hooks": [{"type": "command", "command": cmd(e), "timeout": 10}]}]);
        let named = serde_json::json!({
            "PreToolUse": group("PreToolUse"),
            "PostToolUse": group("PostToolUse"),
            "Stop": [{"type": "command", "command": cmd("Stop"), "timeout": 10}],
        });
        if obj.get("trail") != Some(&named) {
            obj.insert("trail".to_string(), named);
            changed_hooks = true;
        }
    } else {
        if name == "claude" {
            changed_hooks |= add_claude_deny_rules(obj);
        }
        if !obj.contains_key("hooks") || !obj["hooks"].is_object() {
            obj.insert("hooks".to_string(), Value::Object(serde_json::Map::new()));
        }
        let hooks_obj = obj.get_mut("hooks").unwrap().as_object_mut().unwrap();

        // Claude Code also has a prompt hook (where it learns what other agents changed in its files) and a hook for a
        // tool call that failed (PostToolUse is not sent then, so without it the call's window would stay open).
        let events = expected_events(name);
        for event in &events {
            let cmd = format!("trail hook {} --from {}", event, name);
            if !hooks_obj.contains_key(*event) || !hooks_obj[*event].is_array() {
                hooks_obj.insert((*event).to_string(), Value::Array(Vec::new()));
            }
            let event_arr = hooks_obj.get_mut(*event).unwrap().as_array_mut().unwrap();

            let already_has = event_arr.iter().any(|entry| {
                if let Some(inner_hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
                    inner_hooks.iter().any(|h| {
                        h.get("command").and_then(|c| c.as_str()).map(|c| c.contains(&format!("--from {}", name))).unwrap_or(false)
                    })
                } else {
                    false
                }
            });

            if !already_has {
                let hook_entry = serde_json::json!({
                    "matcher": ".*",
                    "hooks": [
                        {
                            "type": "command",
                            "command": cmd
                        }
                    ]
                });
                event_arr.push(hook_entry);
                changed_hooks = true;
            }
        }
    }

    let formatted_json = serde_json::to_string_pretty(&val)?;
    let old_cfg = fs::read_to_string(&cfg_path).unwrap_or_default();
    if !cfg_path.exists() || changed_hooks || old_cfg != formatted_json {
        fs::write(&cfg_path, &formatted_json)?;
    }
    Ok(agent_cfg.to_string())
}

fn install_claude_skill(repo_paths: &RepoPaths) -> Result<String, Box<dyn std::error::Error>> {
    let skill_rel = ".claude/skills/trail/SKILL.md";
    let skill_path = repo_paths.work_tree.join(skill_rel);
    if let Some(parent) = skill_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let old_skill = fs::read_to_string(&skill_path).unwrap_or_default();
    if old_skill != CLAUDE_SKILL_MD {
        fs::write(&skill_path, CLAUDE_SKILL_MD)?;
    }
    Ok(skill_rel.to_string())
}

pub(crate) fn install_agents_md_block(repo_paths: &RepoPaths) -> Result<String, Box<dyn std::error::Error>> {
    let agents_md_path = repo_paths.work_tree.join("AGENTS.md");
    let current_agents_md = if agents_md_path.exists() { fs::read_to_string(&agents_md_path).unwrap_or_default() } else { String::new() };

    let new_agents_md =
        if let (Some(start_idx), Some(end_idx)) = (current_agents_md.find(TRAIL_BLOCK_START), current_agents_md.find(TRAIL_BLOCK_END)) {
            let after_end = end_idx + TRAIL_BLOCK_END.len();
            format!("{}{}{}", &current_agents_md[..start_idx], AGENTS_MD_BLOCK, &current_agents_md[after_end..])
        } else if current_agents_md.trim().is_empty() {
            format!("{}\n", AGENTS_MD_BLOCK)
        } else {
            let mut text = current_agents_md.clone();
            if !text.ends_with('\n') {
                text.push('\n');
            }
            text.push('\n');
            text.push_str(AGENTS_MD_BLOCK);
            text.push('\n');
            text
        };

    if new_agents_md != current_agents_md {
        fs::write(&agents_md_path, &new_agents_md)?;
    }
    Ok("AGENTS.md".to_string())
}

/// Writes everything `agent install` writes for a built-in agent, and prints nothing (the daemon's repair uses it too).
pub(crate) fn install_agent_quiet(repo_paths: &RepoPaths, name: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let Some((_, agent_cfg)) = AGENT_CONFIGS.iter().find(|(a, _)| *a == name) else {
        return Err(format!("unknown agent '{name}'").into());
    };
    let mut written = vec![install_agent_hooks(repo_paths, name, agent_cfg)?];
    if name == "claude" {
        written.push(install_claude_skill(repo_paths)?);
    }
    written.push(install_agents_md_block(repo_paths)?);
    Ok(written)
}

pub fn handle_agent_install(repo_paths: &RepoPaths, name: &str, _scope: &str, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !AGENT_CONFIGS.iter().any(|(a, _)| *a == name) && crate::cmd_adapters::install(repo_paths, name, json)? {
        return Ok(());
    }
    let agent_cfg = match AGENT_CONFIGS.iter().find(|(a, _)| *a == name) {
        Some((_, cfg)) => *cfg,
        None => {
            eprintln!("trail: unknown agent '{name}'. Known agents: claude, codex, gemini, antigravity, cursor");
            std::process::exit(1);
        }
    };

    let _ = agent_cfg;
    let written_files = install_agent_quiet(repo_paths, name)?;
    // the daemon puts these hooks back if they disappear, until `agent uninstall` says otherwise
    crate::self_heal::set_wanted(repo_paths, name, true);

    if json {
        let res = AgentInstallResult { agent: name.to_string(), files: written_files };
        println!("{}", serde_json::to_string(&res)?);
    } else {
        println!("Installed trail adapter for {name}:");
        for f in &written_files {
            println!("  - {f}");
        }
    }

    Ok(())
}

pub fn handle_agent_uninstall(repo_paths: &RepoPaths, name: &str, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !AGENT_CONFIGS.iter().any(|(a, _)| *a == name) && crate::cmd_adapters::uninstall(repo_paths, name, json)? {
        return Ok(());
    }
    let agent_cfg = match AGENT_CONFIGS.iter().find(|(a, _)| *a == name) {
        Some((_, cfg)) => *cfg,
        None => {
            eprintln!("trail: unknown agent '{name}'. Known agents: claude, codex, gemini, antigravity, cursor");
            std::process::exit(1);
        }
    };

    crate::self_heal::set_wanted(repo_paths, name, false);
    // 1. Remove trail hooks from agent config file
    let cfg_path = repo_paths.work_tree.join(agent_cfg);
    if cfg_path.exists() {
        let text = fs::read_to_string(&cfg_path).unwrap_or_default();
        if let Ok(mut val) = serde_json::from_str::<Value>(&text) {
            if let Some(obj) = val.as_object_mut() {
                if name == "antigravity" {
                    obj.remove("trail");
                }
                if let Some(deny) = obj.get_mut("permissions").and_then(|p| p.get_mut("deny")).and_then(|d| d.as_array_mut()) {
                    deny.retain(|d| !d.as_str().is_some_and(|r| CLAUDE_DENY.contains(&r)));
                }
                if let Some(hooks_obj) = obj.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                    let mut empty_events = Vec::new();
                    for (event, event_val) in hooks_obj.iter_mut() {
                        if let Some(arr) = event_val.as_array_mut() {
                            arr.retain(|entry| {
                                if let Some(inner_hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
                                    !inner_hooks.iter().any(|h| {
                                        h.get("command").and_then(|c| c.as_str()).map(|c| c.contains("trail hook")).unwrap_or(false)
                                    })
                                } else {
                                    true
                                }
                            });
                            if arr.is_empty() {
                                empty_events.push(event.clone());
                            }
                        }
                    }
                    for ev in empty_events {
                        hooks_obj.remove(&ev);
                    }
                }
            }
            let formatted = serde_json::to_string_pretty(&val)?;
            fs::write(&cfg_path, formatted)?;
        }
    }

    // 2. Remove Claude skill
    if name == "claude" {
        let skill_path = repo_paths.work_tree.join(".claude/skills/trail/SKILL.md");
        if skill_path.exists() {
            let _ = fs::remove_file(&skill_path);
        }
    }

    // 3. Remove AGENTS.md block
    remove_agents_md_block(repo_paths)?;

    if json {
        let res = AgentUninstallResult { agent: name.to_string(), uninstalled: true };
        println!("{}", serde_json::to_string(&res)?);
    } else {
        println!("Uninstalled trail adapter for {name}");
    }

    Ok(())
}

pub(crate) fn remove_agents_md_block(repo_paths: &RepoPaths) -> Result<(), Box<dyn std::error::Error>> {
    let agents_md_path = repo_paths.work_tree.join("AGENTS.md");
    if agents_md_path.exists() {
        let text = fs::read_to_string(&agents_md_path).unwrap_or_default();
        if let (Some(start_idx), Some(end_idx)) = (text.find(TRAIL_BLOCK_START), text.find(TRAIL_BLOCK_END)) {
            let after_end = end_idx + TRAIL_BLOCK_END.len();
            let mut prefix = text[..start_idx].to_string();
            let suffix = text[after_end..].trim_start_matches('\n');
            if prefix.ends_with("\n\n") && !suffix.is_empty() {
                prefix.truncate(prefix.len() - 1);
            }
            let new_text = format!("{}{}", prefix, suffix);
            fs::write(&agents_md_path, new_text)?;
        }
    }
    Ok(())
}

/// What of Tracewise's capture is actually in place for this repository and this user: per-agent hooks, the shell block,
/// the shims and the git wrapper first on `PATH`, the daemon.
pub struct Coverage {
    pub agents: Vec<(String, bool)>,
    /// Per agent that has hooks: the events still missing (empty when complete).
    pub missing_events: Vec<(String, Vec<&'static str>)>,
    pub shell: bool,
    pub shims: bool,
    pub git_wrapper: bool,
    pub daemon: bool,
}

pub fn coverage(repo_paths: &RepoPaths) -> Coverage {
    let texts: Vec<(&str, String)> =
        AGENT_CONFIGS.iter().map(|(agent, cfg)| (*agent, fs::read_to_string(repo_paths.work_tree.join(cfg)).unwrap_or_default())).collect();
    let has_hooks = |agent: &str, text: &str| text.contains("trail hook") && text.contains(&format!("--from {agent}"));
    let agents = texts.iter().map(|(agent, text)| ((*agent).to_string(), has_hooks(agent, text))).collect();
    let missing_events = texts
        .iter()
        .filter(|(agent, text)| has_hooks(agent, text))
        .map(|(agent, text)| ((*agent).to_string(), missing_events(agent, text)))
        .collect();
    let home = std::env::var("HOME").unwrap_or_default();
    let shell = [".zshenv", ".zshrc", ".bashrc", ".bash_profile", ".trail/bash_env"].iter().any(|f| {
        fs::read_to_string(std::path::Path::new(&home).join(f))
            .map(|t| t.contains("TRAIL_SHELL_ACTIVE") || t.contains("trail shell"))
            .unwrap_or(false)
    });
    let on_path =
        |tool: &str| std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|d| d.join(tool)).find(|p| p.exists());
    let is_ours = |tool: &str| on_path(tool).map(|p| crate::cmd_shims::is_trail_bin(&p, None)).unwrap_or(false);
    Coverage { agents, missing_events, shell, shims: is_ours("sed"), git_wrapper: is_ours("git"), daemon: crate::util::is_daemon_running() }
}

pub fn handle_agent_status(repo_paths: &RepoPaths, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let cov = coverage(repo_paths);
    let shims_installed = cov.shims;
    let agents_md_path = repo_paths.work_tree.join("AGENTS.md");
    let agents_md_has_block = if agents_md_path.exists() {
        let t = fs::read_to_string(&agents_md_path).unwrap_or_default();
        t.contains(TRAIL_BLOCK_START)
    } else {
        false
    };

    let mut entries = Vec::new();
    for (agent, cfg) in &AGENT_CONFIGS {
        let cfg_path = repo_paths.work_tree.join(cfg);
        let has_hooks = if cfg_path.exists() {
            let t = fs::read_to_string(&cfg_path).unwrap_or_default();
            t.contains("trail hook") && t.contains(&format!("--from {}", agent))
        } else {
            false
        };

        let has_skill = if *agent == "claude" {
            let skill_path = repo_paths.work_tree.join(".claude/skills/trail/SKILL.md");
            skill_path.exists() && agents_md_has_block
        } else {
            agents_md_has_block
        };

        entries.push(AgentStatusEntry {
            name: (*agent).to_string(),
            hooks: has_hooks,
            skill: has_skill,
            shims: shims_installed,
            shell: cov.shell,
            missing_events: cov.missing_events.iter().find(|(a, _)| a == agent).map(|(_, m)| m.clone()).unwrap_or_default(),
        });
    }

    if json {
        println!("{}", serde_json::to_string(&entries)?);
    } else {
        println!("{:<15} {:<10} {:<10} {:<10} {:<10}", "AGENT", "HOOKS", "SKILL", "SHIMS", "SHELL");
        for e in &entries {
            println!(
                "{:<15} {:<10} {:<10} {:<10} {:<10}",
                e.name,
                if e.hooks { "yes" } else { "no" },
                if e.skill { "yes" } else { "no" },
                if e.shims { "yes" } else { "no" },
                if e.shell { "yes" } else { "no" }
            );
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "test_cmd_agents.rs"]
mod tests;
