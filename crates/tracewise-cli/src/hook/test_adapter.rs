use super::*;

const DEMO: &str = r#"
[agent]
session = "/sid"
[events]
pointer = "/event"
map = { before = "pre-tool", after = "post-tool", done = "stop", odd = "no-such-event" }
[edit]
tools = ["write_file"]
path = "/args/path"
[command]
tools = ["shell"]
text = "/args/cmd"
[reply]
deny = '{"ok":false,"why":"{reason}"}'
[install]
file = ".demo/hooks.json"
content = '{"hooks":{}}'
"#;

fn demo() -> Adapter {
    let mut a = Adapter::parse("demo", DEMO).unwrap();
    a.edit_tools_ptr = "/tool".into();
    a
}

#[test]
fn a_tool_event_becomes_an_edit_or_a_command_by_its_tool() {
    let a = demo();
    let edit = a.translate("", &serde_json::json!({"event": "before", "sid": "s9", "tool": "write_file", "args": {"path": "a.txt"}}));
    assert_eq!((edit.event.as_deref(), edit.session.as_str(), edit.paths), (Some("pre-edit"), "s9", vec!["a.txt".to_string()]));
    let cmd = a.translate("", &serde_json::json!({"event": "after", "tool": "shell", "args": {"cmd": "ls"}}));
    assert_eq!((cmd.event.as_deref(), cmd.command.as_deref()), (Some("post-command"), Some("ls")));
    assert_eq!(a.translate("", &serde_json::json!({"event": "before", "tool": "other"})).event, None);
}

#[test]
fn unknown_events_and_broken_files_are_not_events() {
    let a = demo();
    assert_eq!(a.translate("", &serde_json::json!({"event": "odd"})).event, None);
    assert_eq!(a.translate("done", &serde_json::json!({})).event.as_deref(), Some("stop"));
    assert!(Adapter::parse("x", "[[[").is_err());
    assert!(Adapter::parse("x", "[install]\nfile = \"f\"\ncontent = \"not json\"").is_err());
}

#[test]
fn the_deny_reply_escapes_the_reason() {
    let a = demo();
    assert_eq!(a.reply(Some("say \"no\"\nnow")), r#"{"ok":false,"why":"say \"no\"\nnow"}"#);
    assert_eq!(a.reply(None), "{}");
}

#[test]
fn only_safe_names_are_looked_up() {
    assert!(is_valid_name("my-agent_2"));
    assert!(!is_valid_name("../x") && !is_valid_name("") && !is_valid_name("a/b"));
    assert_eq!(find("claude", None).unwrap().map(|x| x.1), None);
}
