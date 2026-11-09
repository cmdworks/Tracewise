use super::*;

#[test]
fn test_process_chain_current() {
    let my_pid = std::process::id();
    let chain = process_chain(my_pid);
    assert!(!chain.is_empty(), "process chain should not be empty");
    assert_eq!(chain[0].pid, my_pid, "chain starts with target pid");
    for entry in &chain {
        assert!(!entry.name.is_empty(), "entry name should not be empty");
    }
    let last = chain.last().unwrap();
    assert!(last.pid == 1 || last.pid == 0, "chain ends at launchd (pid 1 or 0), got pid {}", last.pid);
}

#[test]
fn a_writer_is_an_agent_by_signature_name_or_ancestor() {
    let table = AgentTable::new();
    let entry = |pid: u32, name: &str| ProcessEntry { pid, ppid: 0, name: name.to_string(), path: format!("/x/{name}") };
    // the code-signing identity wins over an unhelpful executable name
    assert_eq!(
        agent_for_writer(&table, "/Users/a/.local/share/claude/versions/2.1.309", Some("com.anthropic.claude-code"), 7, &[]),
        Some("claude".to_string())
    );
    // the writer is itself a known agent
    assert_eq!(agent_for_writer(&table, "/opt/bin/codex", None, 7, &[]), Some("codex".to_string()));
    // sed under a shell under codex
    let chain = [entry(5, "zsh"), entry(4, "codex"), entry(1, "launchd")];
    assert_eq!(agent_for_writer(&table, "/usr/bin/sed", Some("com.apple.sed"), 7, &chain), Some("codex".to_string()));
    // nothing agent-like anywhere: a person
    let plain = [entry(5, "zsh"), entry(4, "Terminal"), entry(1, "launchd")];
    assert_eq!(agent_for_writer(&table, "/usr/bin/sed", None, 7, &plain), None);
}

#[test]
fn test_builtin_agent_matching() {
    let table = AgentTable::new();
    assert_eq!(table.match_candidate("claude"), Some("claude".to_string()));
    assert_eq!(table.match_candidate("Claude"), Some("claude".to_string()));
    assert_eq!(table.match_candidate("codex"), Some("codex".to_string()));
    assert_eq!(table.match_candidate("gemini"), Some("gemini".to_string()));
    assert_eq!(table.match_candidate("Antigravity Helper (Renderer)"), Some("antigravity".to_string()));
    assert_eq!(table.match_candidate("cursor-agent"), Some("cursor".to_string()));
    assert_eq!(table.match_candidate("aider"), Some("aider".to_string()));
    assert_eq!(table.match_candidate("bash"), None);
    assert_eq!(table.match_candidate("zsh"), None);
}

#[test]
fn test_agents_toml_parsing_and_override() {
    let toml_content = r#"
# Custom agents configuration
[[agent]]
name = "mybot"
process = ["mybot", "mybot-worker"]

[[agent]]
name = "custom-claude"
process = ["claude"]
"#;
    let table = AgentTable::load_from_content(toml_content);
    // Custom agents win over built-in
    assert_eq!(table.match_candidate("claude"), Some("custom-claude".to_string()));
    assert_eq!(table.match_candidate("mybot-worker"), Some("mybot".to_string()));
    assert_eq!(table.match_candidate("mybot"), Some("mybot".to_string()));
    // Fallback to built-in when not in custom
    assert_eq!(table.match_candidate("codex"), Some("codex".to_string()));
}
