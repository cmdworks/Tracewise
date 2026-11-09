use super::*;

#[test]
fn an_agent_with_and_without_a_session_is_one_agent() {
    let hooked = Actor::agent("claude", "s1");
    let chain = Actor { kind: "agent".into(), name: Some("claude".into()), session: None };
    assert!(hooked.is_same(&chain) && chain.is_same(&hooked));
    assert!(!hooked.is_same(&Actor::agent("claude", "s2")));
    assert!(!hooked.is_same(&Actor::agent("codex", "s1")));
    assert!(!chain.is_same(&Actor::agent("codex", "s1")));
}

#[test]
fn test_cap_command_text_s07() {
    let short = "git checkout main";
    assert_eq!(cap_command_text(short), short);

    let exactly_1k = "a".repeat(1024);
    assert_eq!(cap_command_text(&exactly_1k), exactly_1k);

    let over_1k = "b".repeat(5000);
    let capped = cap_command_text(&over_1k);
    assert_eq!(capped.len(), 1024 + TRUNCATION_MARKER.len());
    assert!(capped.starts_with(&"b".repeat(1024)));
    assert!(capped.ends_with(TRUNCATION_MARKER));

    // UTF-8 multibyte boundary check
    let mut utf8_str = "a".repeat(1023);
    utf8_str.push('🦀'); // 4-byte character across 1024 boundary (1023..1027)
    let capped_utf8 = cap_command_text(&utf8_str);
    assert_eq!(capped_utf8, format!("{}{}", "a".repeat(1023), TRUNCATION_MARKER));
}

#[test]
fn test_actor_parse() {
    assert_eq!(
        Actor::parse("agent:claude:s1"),
        Some(Actor { kind: "agent".to_string(), name: Some("claude".to_string()), session: Some("s1".to_string()) })
    );
    assert_eq!(Actor::parse("human:alice"), Some(Actor { kind: "human".to_string(), name: Some("alice".to_string()), session: None }));
    assert_eq!(Actor::parse("unknown"), Some(Actor { kind: "unknown".to_string(), name: None, session: None }));
    assert_eq!(Actor::parse(""), None);
}

#[test]
fn test_validate_why() {
    assert!(validate_why("fix").is_err());
    assert!(validate_why("Update.").is_err());
    assert!(validate_why("wip").is_err());
    assert!(validate_why("short one").is_err());
    assert!(validate_why("add input validation to the signup form").is_ok());
}

#[test]
fn test_glob_match() {
    assert!(glob_match("src/*.txt", "src/b.txt"));
    assert!(!glob_match("src/*.txt", "other/c.txt"));
    assert!(glob_match("CHANGELOG.md", "CHANGELOG.md"));
    assert!(glob_match("*.rs", "main.rs"));
}

#[test]
fn test_mask_secrets() {
    let m = mask_secrets("curl -H 'Authorization: Bearer ghp_abcdefghijklmnopqrstuvwxyz0123456789' --user admin:hunter2 https://u:pw@h.example AKIAIOSFODNN7EXAMPLE");
    assert!(!mask_secrets("export AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE").contains("AKIAIOSFODNN7EXAMPLE"));
    for leaked in ["ghp_abc", "hunter2", "pw@", "AKIAIOSFODNN7EXAMPLE"] {
        assert!(!m.contains(leaked), "{leaked} leaked in {m}");
    }
    assert!(m.contains("admin:***") && m.contains("u:***@h.example"));
    assert_eq!(mask_secrets("git status --short"), "git status --short");
}
