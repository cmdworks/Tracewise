use super::*;
use tempfile::tempdir;

#[test]
fn test_reasons_mode_parse_and_defaults() {
    assert_eq!(ReasonsMode::parse("off"), Some(ReasonsMode::Off));
    assert_eq!(ReasonsMode::parse("WARN"), Some(ReasonsMode::Warn));
    assert_eq!(ReasonsMode::parse("enforce"), Some(ReasonsMode::Enforce));
    assert_eq!(ReasonsMode::parse("invalid"), None);

    let agent = Actor { kind: "agent".to_string(), name: Some("claude".to_string()), session: None };
    let human = Actor { kind: "human".to_string(), name: Some("alice".to_string()), session: None };

    // Enforce applies to agents, never people
    assert!(ReasonsMode::Enforce.should_enforce(&agent));
    assert!(!ReasonsMode::Enforce.should_enforce(&human));
    assert!(!ReasonsMode::Warn.should_enforce(&agent));
    assert!(!ReasonsMode::Off.should_enforce(&agent));

    // Warn applies to agents, not in off
    assert!(ReasonsMode::Warn.should_warn(&agent));
    assert!(!ReasonsMode::Warn.should_warn(&human));
    assert!(!ReasonsMode::Off.should_warn(&agent));
}

#[test]
fn test_read_reasons_mode_from_tracewiseconfig() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    // Default when no .tracewiseconfig exists
    assert_eq!(read_reasons_mode_from_file(path), None);

    // With .tracewiseconfig containing [reasons]
    fs::write(path.join(".tracewiseconfig"), "[share]\nremote = \"https://example.com/repo.git\"\n\n[reasons]\nmode = \"enforce\"\n")
        .unwrap();

    assert_eq!(read_reasons_mode_from_file(path), Some(ReasonsMode::Enforce));
}

#[test]
fn test_value_in_reads_a_section_key() {
    let text = "[profile]\nname = \"full\" # comment\n[retention]\ncommands = \"7d\"\n";
    assert_eq!(value_in(text, "retention", "commands").as_deref(), Some("7d"));
    assert_eq!(value_in(text, "profile", "name").as_deref(), Some("full"));
    assert_eq!(value_in(text, "retention", "missing"), None);
    assert_eq!(value_in("[[[ broken", "retention", "commands"), None);
}
