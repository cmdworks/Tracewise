use super::{execute_hook, HookInput, HookOutput};
use serde_json::Value;

pub fn handle_claude_hook(agent: &str, cli_event: &str, stdin_str: &str) {
    let val: Value = match serde_json::from_str(stdin_str) {
        Ok(v) => v,
        Err(_) => {
            println!("{{}}");
            return;
        }
    };

    let hook_event = val.get("hook_event_name").and_then(|v| v.as_str()).unwrap_or(cli_event);
    let session_id = val.get("session_id").and_then(|v| v.as_str()).unwrap_or("default");
    let cwd = val.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
    let mk = |paths: Vec<String>, command: Option<String>| HookInput {
        agent: Some(agent.to_string()),
        session: Some(session_id.to_string()),
        paths,
        command,
        cwd: cwd.clone(),
    };
    let allow = || HookOutput { decision: "allow", reason: None, notice: None };
    let tool_name = val.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
    let is_cmd = matches!(tool_name, "Bash" | "bash" | "run_terminal_cmd" | "run_shell_command" | "shell");
    let is_edit =
        matches!(tool_name, "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "edit_file" | "write_file" | "replace" | "apply_patch");
    let cmd_str = || val.get("tool_input").and_then(|t| t.get("command")).and_then(|c| c.as_str()).map(String::from);
    let edit_paths = || {
        let direct: Vec<String> = val
            .get("tool_input")
            .and_then(|t| t.get("file_path").or_else(|| t.get("path")).or_else(|| t.get("target_file")))
            .and_then(|f| f.as_str())
            .map(|fp| vec![fp.to_string()])
            .unwrap_or_default();
        // Codex's apply_patch has no path field: the files are named inside the patch text.
        if direct.is_empty() {
            patch_paths(&cmd_str().unwrap_or_default())
        } else {
            direct
        }
    };

    match hook_event {
        "PreToolUse" if is_cmd || is_edit => {
            let (ev, input) = if is_cmd { ("pre-command", mk(Vec::new(), cmd_str())) } else { ("pre-edit", mk(edit_paths(), None)) };
            let res = execute_hook(ev, input).unwrap_or_else(|_| allow());
            if res.decision == "deny" {
                let out = serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "permissionDecision": "deny",
                        "permissionDecisionReason": res.reason.unwrap_or_default()
                    }
                });
                println!("{}", serde_json::to_string(&out).unwrap());
            } else if let Some(w) = res.notice.filter(|n| agent == "claude" && n.starts_with(crate::hook::events::pre_edit::WARN_PREFIX)) {
                let out = serde_json::json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "additionalContext": w}});
                println!("{}", serde_json::to_string(&out).unwrap());
            } else {
                println!("{{}}");
            }
        }
        "PostToolUse" | "PostToolUseFailure" if is_cmd || is_edit => {
            let (ev, input) = if is_cmd { ("post-command", mk(Vec::new(), cmd_str())) } else { ("post-edit", mk(edit_paths(), None)) };
            let _ = execute_hook(ev, input);
            println!("{{}}");
        }
        "Stop" | "stop" => {
            let res = execute_hook("stop", mk(Vec::new(), None)).unwrap_or_else(|_| allow());
            if res.decision == "deny" || res.decision == "block" {
                println!("{}", serde_json::json!({"decision": "block", "reason": res.reason.unwrap_or_default()}));
            } else {
                println!("{{}}");
            }
        }
        "SessionEnd" | "session-end" => {
            let _ = execute_hook("session-end", mk(Vec::new(), None));
            println!("{{}}");
        }
        "UserPromptSubmit" | "prompt" => {
            let res = execute_hook("prompt", mk(Vec::new(), None)).unwrap_or_else(|_| allow());
            if let Some(notice) = res.notice {
                let out = serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "UserPromptSubmit",
                        "additionalContext": if crate::hook::events::awareness::is_complete(&notice) {
                            notice
                        } else {
                            format!("Tracewise: since your last prompt, other actors changed files you work on:\n{notice}")
                        }
                    }
                });
                println!("{}", serde_json::to_string(&out).unwrap());
            } else {
                println!("{{}}");
            }
        }
        _ => println!("{{}}"),
    }
}

/// Files named by an `apply_patch` body (`*** Add|Update|Delete File: p`, `*** Move to: p`).
fn patch_paths(patch: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in patch.lines() {
        let l = line.trim();
        let p = ["*** Add File: ", "*** Update File: ", "*** Delete File: ", "*** Move to: "].iter().find_map(|k| l.strip_prefix(k));
        if let Some(p) = p.map(str::trim).filter(|p| !p.is_empty()) {
            if !out.iter().any(|o| o == p) {
                out.push(p.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod patch_tests {
    use super::patch_paths;

    #[test]
    fn apply_patch_names_every_file_once() {
        let p = "*** Begin Patch\n*** Update File: /r/a.txt\n@@\n+x\n*** Move to: /r/b.txt\n*** Add File: /r/c.txt\n+y\n*** Delete File: /r/a.txt\n*** End Patch";
        assert_eq!(patch_paths(p), vec!["/r/a.txt", "/r/b.txt", "/r/c.txt"]);
        assert!(patch_paths("echo hi").is_empty());
    }
}
