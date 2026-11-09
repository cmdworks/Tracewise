use super::*;
use tracewise_core::db::Database;
use tracewise_core::model::{Actor, Window};
use tracewise_proc::ProcWhoResult;

#[test]
fn test_agent_session_stopped_then_trail_run_is_person() {
    let _guard = crate::TEST_MUTEX.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    // 1. Agent command window opened and closed (stopped)
    let window = Window {
        id: "cmd1".to_string(),
        kind: "agent_command".to_string(),
        actor: Actor::agent("claude", "s1"),
        paths: vec!["a.txt".to_string()],
        cwd: None,
        start: "2026-10-26T00:00:00.000Z".to_string(),
        end: Some("2026-10-26T00:00:01.000Z".to_string()),
        command: Some("sh -c echo".to_string()),
    };
    db.open_window(&window).unwrap();

    let who = ProcWhoResult {
        kind: "agent".to_string(),
        name: "claude".to_string(),
        session: None,
        source: "process".to_string(),
        matched: None,
    };

    let actor = resolve_run_actor(&db, who);
    assert_eq!(actor.kind, "human");
}

#[test]
fn test_agent_command_window_open_trail_run_is_agent() {
    let _guard = crate::TEST_MUTEX.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    // Agent command window is currently open (end is None)
    let window = Window {
        id: "cmd2".to_string(),
        kind: "agent_command".to_string(),
        actor: Actor::agent("claude", "s1"),
        paths: vec!["a.txt".to_string()],
        cwd: None,
        start: "2026-10-26T00:00:00.000Z".to_string(),
        end: None,
        command: Some("trail run ...".to_string()),
    };
    db.open_window(&window).unwrap();

    let who = ProcWhoResult {
        kind: "agent".to_string(),
        name: "claude".to_string(),
        session: None,
        source: "process".to_string(),
        matched: None,
    };

    let actor = resolve_run_actor(&db, who);
    assert_eq!(actor.kind, "agent");
    assert_eq!(actor.name.as_deref(), Some("claude"));
    assert_eq!(actor.session.as_deref(), Some("s1"));
}

#[test]
fn test_agent_direct_run_without_prior_session_stays_agent() {
    let _guard = crate::TEST_MUTEX.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();

    let who = ProcWhoResult {
        kind: "agent".to_string(),
        name: "claude".to_string(),
        session: None,
        source: "process".to_string(),
        matched: None,
    };

    let actor = resolve_run_actor(&db, who);
    assert_eq!(actor.kind, "agent");
    assert_eq!(actor.name.as_deref(), Some("claude"));
}
