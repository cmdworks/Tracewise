use super::*;
use tempfile::tempdir;
use tracewise_core::model::Window;

fn agent(name: &str) -> Actor {
    Actor { kind: "agent".to_string(), name: Some(name.to_string()), session: Some("s1".to_string()) }
}

fn now_str() -> String {
    tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn window(kind: &str, who: Actor, paths: &[&str], start: String, end: Option<String>) -> Window {
    Window {
        id: ulid::Ulid::new().to_string(),
        kind: kind.to_string(),
        actor: who,
        paths: paths.iter().map(|p| p.to_string()).collect(),
        cwd: None,
        start,
        end,
        command: None,
    }
}

fn setup() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf, Database) {
    let tmp = tempdir().unwrap();
    let work = tmp.path().join("w");
    let data = work.join(".git/tracewise");
    fs::create_dir_all(&data).unwrap();
    let db = Database::open(&data.join("tracewise.db")).unwrap();
    (tmp, work, data, db)
}

/// A change the daemon recorded from file events alone, with nobody to blame. Inserted directly: a scan would read the
/// environment (`CLAUDE_CODE_SESSION_ID` and the like) and attribute the file to whoever runs the tests.
fn watched_by(db: &Database, name: &str, who: Actor, confidence: &str) -> Vec<String> {
    let id = ulid::Ulid::new().to_string();
    let t = now_str();
    let c = Change {
        id: id.clone(),
        time: tracewise_core::model::TimeRange { start: t.clone(), end: t },
        path: name.to_string(),
        op: "modify".to_string(),
        old_path: None,
        hunks: Vec::new(),
        before: None,
        after: None,
        actor: who,
        candidates: Vec::new(),
        confidence: confidence.to_string(),
        source: "watch".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "dev1".to_string(),
        git: tracewise_core::model::GitInfo { branch: "main".to_string(), head: "h".to_string() },
        prev_hash: None,
        undoes: None,
    };
    db.insert_change(&c).unwrap();
    vec![id]
}

fn watched(db: &Database, _work: &Path, _data: &Path, name: &str) -> Vec<String> {
    watched_by(db, name, Actor::unknown(), "unknown")
}

#[test]
fn the_note_names_the_agent_and_the_files_and_says_possibly() {
    let t = text(&[agent("claude")], &["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"]);
    assert_eq!(t, "5 changes possibly by claude with no hook explaining them: a.txt, b.txt, c.txt and 2 more");
    assert_eq!(
        text(&[agent("claude"), agent("codex")], &["a.txt"]),
        "1 change possibly by claude and codex with no hook explaining it: a.txt"
    );
}

#[test]
fn a_write_while_an_agent_was_working_is_noted_once_and_without_touching_the_change() {
    let (_tmp, work, data, db) = setup();
    // claude's last hook window closed a moment ago and covered another file
    db.open_window(&window("agent_edit", agent("claude"), &["other.txt"], now_str(), Some(now_str()))).unwrap();
    let ids = watched(&db, &work, &data, "a.txt");
    assert_eq!(ids.len(), 1);
    let before = db.get_change(&ids[0]).unwrap().unwrap();
    assert_eq!(before.actor.kind, "unknown", "the attribution is the evidence's, not a guess");

    note(&db, &data, &ids);
    let notes: Vec<_> = db.list_commands().unwrap().into_iter().filter(|c| c.kind == "unhooked").collect();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].text.contains("claude") && notes[0].text.contains("a.txt"), "{}", notes[0].text);
    assert_eq!(notes[0].changes, ids);
    let after = db.get_change(&ids[0]).unwrap().unwrap();
    assert_eq!((after.actor.kind.as_str(), after.command.as_deref()), ("unknown", None), "unchanged and not linked to the note");

    let more = watched(&db, &work, &data, "b.txt");
    note(&db, &data, &more);
    assert_eq!(db.list_commands().unwrap().iter().filter(|c| c.kind == "unhooked").count(), 1, "a burst is one note");
}

#[test]
fn nothing_is_noted_without_a_recent_agent_or_for_a_stale_open_window() {
    let (_tmp, work, data, db) = setup();
    let ids = watched(&db, &work, &data, "a.txt");
    note(&db, &data, &ids);
    assert!(db.list_commands().unwrap().iter().all(|c| c.kind != "unhooked"), "no agent at all: a person's save");

    // an agent window that opened an hour ago and never closed is not recent work
    let old = (tracewise_core::clock::now() - chrono::Duration::hours(1)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    db.open_window(&window("agent_command", agent("claude"), &[], old, None)).unwrap();
    let ids = watched(&db, &work, &data, "b.txt");
    note(&db, &data, &ids);
    assert!(db.list_commands().unwrap().iter().all(|c| c.kind != "unhooked"));
}

#[test]
fn a_change_an_agent_window_explains_is_not_unhooked() {
    let (_tmp, work, data, db) = setup();
    db.open_window(&window("agent_edit", agent("claude"), &["a.txt"], now_str(), None)).unwrap();
    // the scan, seeing that window, attributes the file to claude
    let ids = watched_by(&db, "a.txt", agent("claude"), "exact");
    let _ = (&work, &data);
    note(&db, &data, &ids);
    assert!(db.list_commands().unwrap().iter().all(|c| c.kind != "unhooked"), "the hook window named the file");
}
