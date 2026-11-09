use super::helpers::AGY_EDIT_TOOLS;
use super::{execute_hook, HookInput, HookOutput};

pub fn handle_antigravity_hook(cli_event: &str, stdin_str: &str) {
    let val: serde_json::Value = serde_json::from_str(stdin_str).unwrap_or_default();
    let session = val.get("conversationId").and_then(|v| v.as_str()).unwrap_or("default").to_string();
    let cwd = val.pointer("/workspacePaths/0").and_then(|v| v.as_str()).map(String::from);
    let tool = val.pointer("/toolCall/name").and_then(|v| v.as_str()).unwrap_or("");
    let arg =
        |keys: &[&str]| keys.iter().find_map(|k| val.pointer(&format!("/toolCall/args/{k}")).and_then(|v| v.as_str())).map(String::from);
    let mk = |paths: Vec<String>, command: Option<String>| HookInput {
        agent: Some("antigravity".to_string()),
        session: Some(session.clone()),
        paths,
        command,
        cwd: cwd.clone(),
    };
    let allow = || HookOutput { decision: "allow", reason: None, notice: None };
    let reply = |res: HookOutput| {
        if res.decision == "deny" {
            println!("{}", serde_json::json!({"decision": "deny", "reason": res.reason.unwrap_or_default()}));
        } else {
            println!(r#"{{"decision":"allow"}}"#);
        }
    };
    let is_edit = AGY_EDIT_TOOLS.contains(&tool);
    let edit_paths = || arg(&["TargetFile", "Path", "AbsolutePath", "FilePath", "File"]).into_iter().collect::<Vec<_>>();
    match cli_event {
        "PreToolUse" if is_edit => reply(execute_hook("pre-edit", mk(edit_paths(), None)).unwrap_or_else(|_| allow())),
        "PreToolUse" if tool == "run_command" => {
            reply(execute_hook("pre-command", mk(Vec::new(), arg(&["CommandLine"]))).unwrap_or_else(|_| allow()))
        }
        "PostToolUse" if is_edit => {
            let _ = execute_hook("post-edit", mk(edit_paths(), None));
            println!("{{}}");
        }
        "PostToolUse" if tool == "run_command" => {
            let _ = execute_hook("post-command", mk(Vec::new(), arg(&["CommandLine"])));
            println!("{{}}");
        }
        "Stop" => {
            let ev = if val.get("fullyIdle").and_then(|v| v.as_bool()) == Some(true) { "session-end" } else { "stop" };
            let _ = execute_hook(ev, mk(Vec::new(), None));
            println!(r#"{{"decision":"allow"}}"#);
        }
        // agy rejects any field in a PostToolUse reply: it expects `{}`
        "PostToolUse" => println!("{{}}"),
        _ => println!(r#"{{"decision":"allow"}}"#),
    }
}
