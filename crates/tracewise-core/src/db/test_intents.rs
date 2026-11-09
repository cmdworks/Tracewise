use super::*;

#[test]
fn test_intent_coverage_for_agent_without_session() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("tracewise.db");
    let db = Database::open(&db_path).unwrap();

    // Intent with agent name and NO session
    let intent = Intent {
        id: "int-1".to_string(),
        actor: Actor { kind: "agent".to_string(), name: Some("codex".to_string()), session: None },
        paths: vec!["src/*.rs".to_string()],
        why: "refactor".to_string(),
        task: None,
        created: "2026-10-23T10:00:00.000Z".to_string(),
        expires: (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        kind: "intent".to_string(),
        ended: 0,
    };
    db.insert_intent(&intent).unwrap();

    // Query with session None
    let cov1 = db.find_covering_intent("codex", None, "src/main.rs").unwrap();
    assert!(cov1.is_some());
    assert_eq!(cov1.unwrap().id, "int-1");

    // Query with session Some("s1") - intent with no session covers it!
    let cov2 = db.find_covering_intent("codex", Some("s1"), "src/main.rs").unwrap();
    assert!(cov2.is_some());
    assert_eq!(cov2.unwrap().id, "int-1");

    // Query for different agent - does not match
    let cov3 = db.find_covering_intent("claude", None, "src/main.rs").unwrap();
    assert!(cov3.is_none());
}
