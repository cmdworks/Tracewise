use super::actor::resolve_actor_candidates_confidence;
use super::*;
use crate::db::Database;
use crate::model::Actor;
use std::fs;
use std::path::PathBuf;

#[test]
fn a_shell_that_exited_does_not_make_later_changes_ambiguous() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();
    let mut child = std::process::Command::new("true").spawn().unwrap();
    child.wait().unwrap();
    let dead_pid = child.id();
    let a_minute_ago = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let window = |id: String, kind: &str, actor: Actor| crate::model::Window {
        id,
        kind: kind.to_string(),
        actor,
        paths: Vec::new(),
        cwd: None,
        start: a_minute_ago.clone(),
        end: None,
        command: None,
    };
    let me = Actor { kind: "human".to_string(), name: Some("asmat".to_string()), session: None };
    db.open_window(&window(format!("shell_{dead_pid}"), "shell", me)).unwrap();
    db.open_window(&window("agent_cmd1".to_string(), "agent_command", Actor::agent("claude", "s1"))).unwrap();
    let options = ScanOptions { actor: Some(Actor::agent("claude", "s1")), ..ScanOptions::default() };
    let (actor, candidates, _, _) = resolve_actor_candidates_confidence(&db, &options, "a.txt", false);
    assert_eq!(actor, Actor::agent("claude", "s1"));
    assert!(candidates.is_empty());
    let open: Vec<_> = db
        .get_overlapping_windows("2000-01-01T00:00:00.000Z", "2999-01-01T00:00:00.000Z")
        .unwrap()
        .into_iter()
        .filter(|w| w.end.is_none())
        .map(|w| w.id)
        .collect();
    assert_eq!(open, vec!["agent_cmd1".to_string()]);
}

#[test]
fn a_background_window_is_the_weakest_evidence() {
    let _env_guard = crate::test_env::ENV_LOCK.lock().unwrap();
    crate::test_env::clear_agent_markers();
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();
    let ago = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let later = |secs: i64| (chrono::Utc::now() + chrono::Duration::seconds(secs)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let window = |id: &str, kind: &str, actor: Actor, end: Option<String>| crate::model::Window {
        id: id.to_string(),
        kind: kind.to_string(),
        actor,
        paths: vec!["*".to_string()],
        cwd: None,
        start: ago.clone(),
        end,
        command: None,
    };
    let options = ScanOptions::default();

    db.open_window(&window("bg1", "agent_background", Actor::agent("claude", "s1"), Some(later(60)))).unwrap();
    let (actor, _, conf, source) = resolve_actor_candidates_confidence(&db, &options, "a.txt", false);
    assert_eq!(actor, Actor::agent("claude", "s1"));
    assert_eq!((conf.as_str(), source.as_deref()), ("likely", Some("background")));

    db.open_window(&window("bg2", "agent_background", Actor::agent("codex", "c1"), Some(later(60)))).unwrap();
    let (actor, candidates, _, _) = resolve_actor_candidates_confidence(&db, &options, "a.txt", false);
    assert_eq!((actor.kind.as_str(), candidates.len()), ("ambiguous", 2));

    db.open_window(&window("w1", "agent_command", Actor::agent("gemini", "g1"), None)).unwrap();
    let (actor, _, _, source) = resolve_actor_candidates_confidence(&db, &options, "a.txt", false);
    assert_eq!(actor.kind, "unknown");
    assert_eq!(source, None);

    let temp2 = tempfile::tempdir().unwrap();
    let db2 = Database::open(&temp2.path().join("trail.db")).unwrap();
    db2.open_window(&window("bg3", "agent_background", Actor::agent("claude", "s1"), Some(later(-5)))).unwrap();
    let (actor, _, _, _) = resolve_actor_candidates_confidence(&db2, &options, "a.txt", false);
    assert_eq!(actor.kind, "unknown");
    db2.open_window(&window("bg4", "agent_background", Actor::agent("claude", "s1"), Some(later(60)))).unwrap();
    let with_actor = ScanOptions { actor: Some(Actor::agent("codex", "c1")), ..ScanOptions::default() };
    let (actor, _, _, _) = resolve_actor_candidates_confidence(&db2, &with_actor, "a.txt", false);
    assert_eq!(actor, Actor::agent("codex", "c1"));
}

#[test]
fn one_person_seen_by_two_hooks_is_not_ambiguous() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();
    let a_minute_ago = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let window = |id: &str, kind: &str, actor: Actor| crate::model::Window {
        id: id.to_string(),
        kind: kind.to_string(),
        actor,
        paths: Vec::new(),
        cwd: None,
        start: a_minute_ago.clone(),
        end: None,
        command: None,
    };
    let me = Actor { kind: "human".to_string(), name: Some("asmat".to_string()), session: None };
    let nameless = Actor { kind: "human".to_string(), name: None, session: None };
    db.open_window(&window("shell1", "shell", me.clone())).unwrap();
    db.open_window(&window("shim1", "shim", nameless.clone())).unwrap();
    let options = ScanOptions { actor: Some(nameless), ..ScanOptions::default() };
    let (actor, candidates, _, _) = resolve_actor_candidates_confidence(&db, &options, "a.txt", false);
    assert_eq!(actor, me, "the person keeps their name");
    assert!(candidates.is_empty());

    db.open_window(&window("agent1", "agent_command", Actor::agent("claude", "s1"))).unwrap();
    let (actor, candidates, _, _) = resolve_actor_candidates_confidence(&db, &options, "a.txt", false);
    assert_eq!(actor.kind, "ambiguous");
    assert_eq!(candidates.len(), 2);
}

#[test]
fn test_scan_create_modify_delete_rename() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data_dir = repo.join(".git/trail");
    fs::create_dir_all(&data_dir).unwrap();

    // 1. Initial file
    fs::write(repo.join("file1.txt"), "first line\n").unwrap();
    let files = vec![PathBuf::from("file1.txt")];
    let changes = scan_repository(&repo, &data_dir, "main", "12345678", &ScanOptions::default(), &files).unwrap();
    assert_eq!(changes.len(), 1);

    // 2. Modify file
    fs::write(repo.join("file1.txt"), "first line\nsecond line\n").unwrap();
    let changes = scan_repository(&repo, &data_dir, "main", "12345678", &ScanOptions::default(), &files).unwrap();
    assert_eq!(changes.len(), 1);

    // 3. Rename file
    fs::remove_file(repo.join("file1.txt")).unwrap();
    fs::write(repo.join("file2.txt"), "first line\nsecond line\n").unwrap();
    let files2 = vec![PathBuf::from("file2.txt")];
    let changes = scan_repository(&repo, &data_dir, "main", "12345678", &ScanOptions::default(), &files2).unwrap();
    assert_eq!(changes.len(), 1);

    let db = Database::open(&data_dir.join("trail.db")).unwrap();
    let rename_rec = db.get_change(&changes[0]).unwrap().unwrap();
    assert_eq!(rename_rec.op, "rename");
    assert_eq!(rename_rec.old_path, Some("file1.txt".to_string()));
    assert_eq!(rename_rec.path, "file2.txt");

    // 4. Delete file
    fs::remove_file(repo.join("file2.txt")).unwrap();
    let changes = scan_repository(&repo, &data_dir, "main", "12345678", &ScanOptions::default(), &[]).unwrap();
    assert_eq!(changes.len(), 1);
    let del_rec = db.get_change(&changes[0]).unwrap().unwrap();
    assert_eq!(del_rec.op, "delete");
}

#[test]
fn test_scan_specific_paths() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data_dir = repo.join(".git/trail");
    fs::create_dir_all(&data_dir).unwrap();

    fs::write(repo.join("a.txt"), "hello\n").unwrap();
    let changes = scan_specific_paths(
        &repo,
        &data_dir,
        "main",
        "12345678",
        &ScanOptions {
            source: "hook".to_string(),
            actor: Some(Actor::agent("claude", "s1")),
            confidence: Some("exact".to_string()),
            predicted_paths: None,
            window_start: None,
            op: None,
            undoes: None,
            path_undoes: None,
        },
        &[PathBuf::from("a.txt")],
    )
    .unwrap();
    assert_eq!(changes.len(), 1);

    let db = Database::open(&data_dir.join("trail.db")).unwrap();
    let rec = db.get_change(&changes[0]).unwrap().unwrap();
    assert_eq!(rec.actor, Actor::agent("claude", "s1"));
    assert_eq!(rec.confidence, "exact");
    assert_eq!(rec.source, "hook");
}

#[test]
fn unattributed_modify_without_agent_window_is_human_likely() {
    let _env_guard = crate::test_env::ENV_LOCK.lock().unwrap();
    crate::test_env::clear_agent_markers();
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();
    let options = ScanOptions::default();

    // Modify with no active window attributes to human likely
    let (actor, candidates, conf, _) = resolve_actor_candidates_confidence(&db, &options, "f.txt", true);
    assert_eq!(actor.kind, "human");
    assert_eq!(conf, "likely");
    assert!(candidates.is_empty());

    // Create with no active window stays unknown
    let (create_actor, _, create_conf, _) = resolve_actor_candidates_confidence(&db, &options, "f.txt", false);
    assert_eq!(create_actor.kind, "unknown");
    assert_eq!(create_conf, "unknown");
}

/// The same operations through a full scan and a path scan must record the same thing.
#[test]
fn full_scan_and_path_scan_record_the_same_with_an_ignored_file() {
    let record = |targeted: bool| -> Vec<(String, String)> {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let data_dir = repo.join(".git/trail");
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
        fs::write(repo.join("a.txt"), "same content\n").unwrap();
        // a full scan is handed what the walker found (ignored files are not in it); a path scan is handed the event paths
        let scan = |event_paths: &[&str], walked: &[&str]| {
            let list = |v: &[&str]| v.iter().map(PathBuf::from).collect::<Vec<_>>();
            if targeted {
                scan_specific_paths(&repo, &data_dir, "main", "1", &ScanOptions::default(), &list(event_paths)).unwrap();
            } else {
                scan_repository(&repo, &data_dir, "main", "1", &ScanOptions::default(), &list(walked)).unwrap();
            }
        };
        scan(&[".gitignore", "a.txt"], &[".gitignore", "a.txt"]);
        // a.txt vanishes while an ignored file with the same content appears: that is a delete, never a rename
        fs::remove_file(repo.join("a.txt")).unwrap();
        fs::write(repo.join("noise.log"), "same content\n").unwrap();
        scan(&["a.txt", "noise.log"], &[".gitignore"]);
        let db = Database::open(&data_dir.join("trail.db")).unwrap();
        let mut out: Vec<(String, String)> = db.list_changes(None, None).unwrap().into_iter().map(|c| (c.op, c.path)).collect();
        out.sort();
        out
    };
    let full = record(false);
    let targeted = record(true);
    assert_eq!(full, targeted);
    assert!(targeted.iter().all(|(op, path)| op != "rename" && path != "noise.log"), "{targeted:?}");
}

/// a blob that cannot be written must not leave a change pointing at it; the next scan records the file.
#[test]
fn a_failed_blob_write_records_nothing_and_the_next_scan_recovers() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data_dir = repo.join(".git/trail");
    let objects = data_dir.join("objects");
    fs::create_dir_all(&data_dir).unwrap();
    fs::create_dir_all(&objects).unwrap();
    fs::write(repo.join("a.txt"), "hello\n").unwrap();
    let files = vec![PathBuf::from("a.txt")];

    if tracewise_platform::private::set_mode(&objects, 0o555).is_err() {
        return; // no file modes on this platform
    }
    if fs::create_dir(objects.join("probe")).is_ok() {
        return; // running as root: the folder cannot be made read-only
    }
    let full = scan_repository(&repo, &data_dir, "main", "1", &ScanOptions::default(), &files);
    let targeted = scan_specific_paths(&repo, &data_dir, "main", "1", &ScanOptions::default(), &files);
    assert!(full.is_err() && targeted.is_err(), "{full:?} {targeted:?}");
    let db = Database::open(&data_dir.join("trail.db")).unwrap();
    assert!(db.list_changes(None, None).unwrap().is_empty());
    assert!(db.get_file("a.txt").unwrap().is_none());

    tracewise_platform::private::set_mode(&objects, 0o755).ok();
    let ids = scan_repository(&repo, &data_dir, "main", "1", &ScanOptions::default(), &files).unwrap();
    assert_eq!(ids.len(), 1);
}

/// `verify` must notice a recorded text version whose blob is gone, and not blame hash-only files.
#[test]
fn a_deleted_blob_is_reported_but_a_hash_only_file_is_not() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data_dir = repo.join(".git/trail");
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(repo.join("a.txt"), "hello\n").unwrap();
    fs::write(repo.join("b.bin"), [0u8, 1, 2, 3]).unwrap();
    let files = vec![PathBuf::from("a.txt"), PathBuf::from("b.bin")];
    let ids = scan_repository(&repo, &data_dir, "main", "1", &ScanOptions::default(), &files).unwrap();
    assert_eq!(ids.len(), 2);

    let db = Database::open(&data_dir.join("trail.db")).unwrap();
    let blobs = crate::blob::BlobStore::new(&data_dir);
    assert!(db.changes_with_missing_blobs(&blobs).unwrap().is_empty());

    let a_change = db.list_changes(Some("a.txt"), None).unwrap().remove(0);
    let hash = a_change.after.clone().unwrap();
    fs::remove_file(data_dir.join("objects").join(&hash[..2]).join(&hash[2..])).unwrap();
    assert_eq!(db.changes_with_missing_blobs(&blobs).unwrap(), vec![a_change.id.clone()]);

    // another device's change with no content (a `summary` batch) is not reported
    let mut theirs = a_change;
    theirs.id = "01OTHERDEVICE".to_string();
    theirs.device = "dev-someone-else".to_string();
    theirs.after = Some("0".repeat(64));
    assert!(db.insert_change_imported(&theirs).unwrap());
    assert_eq!(db.changes_with_missing_blobs(&blobs).unwrap().len(), 1);
}

/// B07: a file that cannot be inspected is not a deleted file.
#[test]
fn an_unreadable_folder_is_not_recorded_as_a_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data_dir = repo.join(".git/trail");
    fs::create_dir_all(&data_dir).unwrap();
    fs::create_dir_all(repo.join("sub")).unwrap();
    fs::write(repo.join("sub/a.txt"), "hello\n").unwrap();
    let paths = vec![PathBuf::from("sub/a.txt")];
    scan_specific_paths(&repo, &data_dir, "main", "1", &ScanOptions::default(), &paths).unwrap();

    tracewise_platform::private::set_mode(repo.join("sub"), 0o000).ok();
    let denied = fs::symlink_metadata(repo.join("sub/a.txt")).is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
    let result = scan_specific_paths(&repo, &data_dir, "main", "1", &ScanOptions::default(), &paths);
    tracewise_platform::private::set_mode(repo.join("sub"), 0o755).ok();
    if !denied {
        return; // running as root: the folder cannot be made unreadable
    }
    assert!(result.is_err(), "{result:?}");
    let db = Database::open(&data_dir.join("trail.db")).unwrap();
    assert_eq!(db.list_changes(Some("sub/a.txt"), None).unwrap().len(), 1, "only the creation, no delete");
}

/// a full scan treats a file in an unreadable folder like the path scan does: unknown, not deleted.
#[test]
fn a_full_scan_keeps_files_of_an_unreadable_folder() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data_dir = repo.join(".git/trail");
    fs::create_dir_all(&data_dir).unwrap();
    fs::create_dir_all(repo.join("sub")).unwrap();
    fs::write(repo.join("sub/a.txt"), "hello\n").unwrap();
    let opts = ScanOptions::default();
    scan_repository(&repo, &data_dir, "main", "1", &opts, &[PathBuf::from("sub/a.txt")]).unwrap();

    tracewise_platform::private::set_mode(repo.join("sub"), 0o000).ok();
    let denied = fs::symlink_metadata(repo.join("sub/a.txt")).is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
    let result = scan_repository(&repo, &data_dir, "main", "1", &opts, &[]);
    tracewise_platform::private::set_mode(repo.join("sub"), 0o755).ok();
    if !denied {
        return; // running as root
    }
    result.unwrap();
    let db = Database::open(&data_dir.join("trail.db")).unwrap();
    assert_eq!(db.list_changes(Some("sub/a.txt"), None).unwrap().len(), 1, "only the creation, no delete");

    fs::remove_dir_all(repo.join("sub")).unwrap();
    scan_repository(&repo, &data_dir, "main", "1", &opts, &[]).unwrap();
    assert_eq!(db.list_changes(Some("sub/a.txt"), None).unwrap().len(), 2, "a real removal is still a delete");
}

/// a file holding a known secret is recorded by hash only: the key is in no blob, no hunk and no line map.
#[test]
fn a_file_with_a_secret_is_recorded_by_hash_only() {
    for targeted in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let data_dir = repo.join(".git/trail");
        fs::create_dir_all(&data_dir).unwrap();
        let key = format!("ghp_{}", "A1b2C3d4E5".repeat(4));
        fs::write(repo.join("cfg.txt"), format!("token = {key}\n")).unwrap();
        fs::write(repo.join("plain.txt"), "hello\n").unwrap();
        let files = vec![PathBuf::from("cfg.txt"), PathBuf::from("plain.txt")];
        if targeted {
            scan_specific_paths(&repo, &data_dir, "main", "1", &ScanOptions::default(), &files).unwrap();
        } else {
            scan_repository(&repo, &data_dir, "main", "1", &ScanOptions::default(), &files).unwrap();
        }
        let db = Database::open(&data_dir.join("trail.db")).unwrap();
        let cfg = db.list_changes(Some("cfg.txt"), None).unwrap();
        assert_eq!(cfg.len(), 1, "the change itself is recorded (targeted={targeted})");
        assert!(cfg[0].hunks.is_empty() && cfg[0].after.is_some());
        let blobs = crate::blob::BlobStore::new(&data_dir);
        assert!(blobs.read(cfg[0].after.as_ref().unwrap()).is_err(), "no stored version");
        assert!(db.get_file_line_map("cfg.txt").unwrap().is_none(), "no line map");
        let plain = db.list_changes(Some("plain.txt"), None).unwrap();
        assert!(!plain[0].hunks.is_empty() && blobs.read(plain[0].after.as_ref().unwrap()).is_ok(), "other files are stored as before");
        // no object file anywhere holds the key
        let mut stack = vec![data_dir.clone()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if let Ok(b) = fs::read(&p) {
                    assert!(!String::from_utf8_lossy(&b).contains(&key), "key found in {p:?}");
                }
            }
        }
    }
}

/// both scans stamp the per-path undo target (`path_undoes`) on the records they build.
#[test]
fn full_and_path_scan_both_record_the_per_path_undo_target() {
    for targeted in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let data_dir = repo.join(".git/trail");
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(repo.join("a.txt"), "a\n").unwrap();
        let files = vec![PathBuf::from("a.txt")];
        let scan = |opts: &ScanOptions| {
            if targeted {
                scan_specific_paths(&repo, &data_dir, "main", "1", opts, &files).unwrap()
            } else {
                scan_repository(&repo, &data_dir, "main", "1", opts, &files).unwrap()
            }
        };
        scan(&ScanOptions::default());
        fs::write(repo.join("a.txt"), "a\nb\n").unwrap();
        let opts = ScanOptions { path_undoes: Some([("a.txt".to_string(), vec!["U1".to_string()])].into()), ..ScanOptions::default() };
        scan(&opts);
        let db = Database::open(&data_dir.join("trail.db")).unwrap();
        let last = db.list_changes(Some("a.txt"), None).unwrap().into_iter().find(|c| c.op == "modify").unwrap();
        assert_eq!(last.undoes, Some(vec!["U1".to_string()]), "targeted={targeted}");
    }
}

/// G03: the same edits recorded through a full scan and through a path scan give the same records.
#[test]
fn full_scan_and_path_scan_agree_on_create_modify_delete_chmod_and_links() {
    // (files before, edit applied, paths the edit touched)
    type Edit = fn(&std::path::Path);
    let scenarios: Vec<(&str, Edit, &[&str])> = vec![
        ("create", |r| fs::write(r.join("new.txt"), "n\n").unwrap(), &["new.txt"]),
        ("modify", |r| fs::write(r.join("a.txt"), "a\nmore\n").unwrap(), &["a.txt"]),
        ("delete", |r| fs::remove_file(r.join("a.txt")).unwrap(), &["a.txt"]),
        ("chmod", |r| tracewise_platform::private::set_mode(r.join("a.txt"), 0o755).ok().unwrap_or_default(), &["a.txt"]),
        ("symlink", |r| tracewise_platform::private::symlink("a.txt", r.join("link")).ok().unwrap_or_default(), &["link"]),
        ("rename", |r| fs::rename(r.join("a.txt"), r.join("b.txt")).unwrap(), &["a.txt", "b.txt"]),
    ];
    for (name, edit, touched) in scenarios {
        let record = |targeted: bool| -> Vec<(String, String)> {
            let temp = tempfile::tempdir().unwrap();
            let repo = temp.path().join("repo");
            let data_dir = repo.join(".git/trail");
            fs::create_dir_all(&data_dir).unwrap();
            fs::write(repo.join("a.txt"), "a\n").unwrap();
            let all = |r: &std::path::Path| -> Vec<PathBuf> {
                let mut v: Vec<PathBuf> =
                    fs::read_dir(r).unwrap().flatten().filter(|e| e.file_name() != ".git").map(|e| PathBuf::from(e.file_name())).collect();
                v.sort();
                v
            };
            let opts = ScanOptions::default();
            scan_repository(&repo, &data_dir, "main", "1", &opts, &all(&repo)).unwrap();
            edit(&repo);
            if targeted {
                let list: Vec<PathBuf> = touched.iter().map(PathBuf::from).collect();
                scan_specific_paths(&repo, &data_dir, "main", "1", &opts, &list).unwrap();
            } else {
                scan_repository(&repo, &data_dir, "main", "1", &opts, &all(&repo)).unwrap();
            }
            let db = Database::open(&data_dir.join("trail.db")).unwrap();
            let mut out: Vec<(String, String)> = db.list_changes(None, None).unwrap().into_iter().map(|c| (c.op, c.path)).collect();
            out.sort();
            out
        };
        assert_eq!(record(false), record(true), "scenario {name}");
    }
}

#[test]
fn an_agent_window_beats_a_long_running_shell_command() {
    let _env_guard = crate::test_env::ENV_LOCK.lock().unwrap();
    crate::test_env::clear_agent_markers();
    let temp = tempfile::tempdir().unwrap();
    let db = Database::open(&temp.path().join("trail.db")).unwrap();
    let ago = |mins: i64| (chrono::Utc::now() - chrono::Duration::minutes(mins)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let me = Actor { kind: "human".to_string(), name: Some("asmat".to_string()), session: None };
    let window = |id: String, kind: &str, actor: Actor, start: String| crate::model::Window {
        id,
        kind: kind.to_string(),
        actor,
        paths: Vec::new(),
        cwd: None,
        start,
        end: None,
        command: None,
    };
    // `npm start`, open for 10 minutes, owned by this test process so it counts as alive
    db.open_window(&window(format!("shell_{}", std::process::id()), "shell", me.clone(), ago(10))).unwrap();
    let options = ScanOptions { actor: Some(Actor::agent("codex", "c1")), ..ScanOptions::default() };
    let (actor, candidates, _, _) = resolve_actor_candidates_confidence(&db, &options, "a.txt", true);
    assert_eq!((actor, candidates.len()), (Actor::agent("codex", "c1"), 0));

    // with no agent as a candidate the shell window still counts, as before
    let (actor, _, _, _) = resolve_actor_candidates_confidence(&db, &ScanOptions::default(), "a.txt", true);
    assert_eq!(actor.kind, "unknown");
}
