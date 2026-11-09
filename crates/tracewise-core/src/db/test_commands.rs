use super::*;
use crate::db::{parse_retention_duration, read_command_retention};

#[test]
fn test_commands_crud() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("tracewise.db");
    let db = Database::open(&db_path).unwrap();

    let cmd = CommandRecord {
        id: "cmd1".to_string(),
        kind: "shell".to_string(),
        text: "cargo test".to_string(),
        cwd: Some("/test".to_string()),
        actor: Actor { kind: "human".to_string(), name: Some("alice".to_string()), session: None },
        start: "2026-10-23T10:00:00Z".to_string(),
        end: "2026-10-23T10:00:05Z".to_string(),
        exit_code: 0,
        changes: vec!["chg1".to_string()],
    };

    db.insert_command(&cmd).unwrap();
    let list = db.list_commands().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "cmd1");
    assert_eq!(list[0].text, "cargo test");
    assert_eq!(list[0].changes, vec!["chg1"]);
}

#[test]
fn test_insert_command_caps_text_s07() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("tracewise.db")).unwrap();

    let long_text = format!("git commit -m \"{}\"", "x".repeat(5000));
    let cmd = CommandRecord {
        id: "cmd_long".to_string(),
        kind: "git".to_string(),
        text: long_text,
        cwd: Some("/repo".to_string()),
        actor: Actor { kind: "human".to_string(), name: Some("asmat".to_string()), session: None },
        start: "2026-10-25T12:00:00.000Z".to_string(),
        end: "2026-10-25T12:00:01.000Z".to_string(),
        exit_code: 0,
        changes: vec![],
    };

    db.insert_command(&cmd).unwrap();
    let list = db.list_commands().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].text.len(), 1024 + crate::model::TRUNCATION_MARKER.len());
    assert!(list[0].text.ends_with(crate::model::TRUNCATION_MARKER));
}

#[test]
fn test_retention_duration_parsing_and_config() {
    assert_eq!(parse_retention_duration("30d"), Some(chrono::Duration::days(30)));
    assert_eq!(parse_retention_duration("7d"), Some(chrono::Duration::days(7)));
    assert_eq!(parse_retention_duration("2w"), Some(chrono::Duration::weeks(2)));
    assert_eq!(parse_retention_duration("24h"), Some(chrono::Duration::hours(24)));
    assert_eq!(parse_retention_duration("1m"), Some(chrono::Duration::days(30)));
    assert_eq!(parse_retention_duration("1y"), Some(chrono::Duration::days(365)));
    assert_eq!(parse_retention_duration("all"), None);
    assert_eq!(parse_retention_duration("none"), None);
    assert_eq!(parse_retention_duration("off"), None);
    assert_eq!(parse_retention_duration("0"), None);

    // Test reading from .tracewiseconfig
    let temp = tempfile::tempdir().unwrap();
    let wt = temp.path();

    // Default 30 days when unset
    assert_eq!(read_command_retention(wt), Some(chrono::Duration::days(30)));

    // Explicit 7d in .tracewiseconfig
    std::fs::write(wt.join(".tracewiseconfig"), "[retention]\ncommands = 7d\n").unwrap();
    assert_eq!(read_command_retention(wt), Some(chrono::Duration::days(7)));

    // Disabled via 'all'
    std::fs::write(wt.join(".tracewiseconfig"), "[retention]\ncommands = all\n").unwrap();
    assert_eq!(read_command_retention(wt), None);
}

#[test]
fn test_purge_expired_commands_s08_s09() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("tracewise.db");
    let db = Database::open(&db_path).unwrap();

    let now = chrono::Utc::now();
    let old_time = (now - chrono::Duration::days(40)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let recent_time = (now - chrono::Duration::days(10)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    // Insert old command (>30 days old)
    let old_cmd = CommandRecord {
        id: "old1".to_string(),
        kind: "shell".to_string(),
        text: "git status".to_string(),
        cwd: None,
        actor: Actor::unknown(),
        start: old_time.clone(),
        end: old_time,
        exit_code: 0,
        changes: vec![],
    };
    db.insert_command(&old_cmd).unwrap();

    // Insert recent command (10 days old)
    let recent_cmd = CommandRecord {
        id: "rec1".to_string(),
        kind: "shell".to_string(),
        text: "git diff".to_string(),
        cwd: None,
        actor: Actor::unknown(),
        start: recent_time.clone(),
        end: recent_time,
        exit_code: 0,
        changes: vec![],
    };
    db.insert_command(&recent_cmd).unwrap();

    assert_eq!(db.list_commands().unwrap().len(), 2);

    // Prune older than 30 days
    let pruned = db.purge_commands_older_than(chrono::Duration::days(30)).unwrap();
    assert_eq!(pruned, 1);

    let remaining = db.list_commands().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "rec1");
}
