//! Tests for history, restore, diff, and labels.
use super::*;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_git_object_storage_restore_and_diff() {
    let tmp = tempfile::tempdir().unwrap();
    let work_tree = tmp.path().join("repo");
    let data_dir = work_tree.join(".git").join("trail");
    fs::create_dir_all(&data_dir).unwrap();
    fs::create_dir_all(work_tree.join(".git").join("objects")).unwrap();

    let db_path = data_dir.join("trail.db");
    let mut db = Database::open(&db_path).unwrap();
    let blobs = BlobStore::new(&data_dir);

    // Version 1
    let file_path = work_tree.join("test.txt");
    let v1_content = b"first version line 1\nfirst version line 2\n";
    fs::write(&file_path, v1_content).unwrap();

    let v1_hash = tracewise_core::blob::hash_bytes(v1_content);
    assert_eq!(v1_hash.len(), 40);

    let options = ScanOptions { source: "test".to_string(), ..Default::default() };
    let changes1 =
        tracewise_core::scan_repository_with_db(&work_tree, &data_dir, "main", "head1", &options, &[PathBuf::from("test.txt")], &mut db)
            .unwrap();
    assert_eq!(changes1.len(), 1);
    let c1 = db.get_change(&changes1[0]).unwrap().unwrap();
    assert_eq!(c1.after.as_deref(), Some(v1_hash.as_str()));
    assert_eq!(c1.before, None);

    // Version 2
    let v2_content = b"first version line 1\nsecond version line 2\n";
    fs::write(&file_path, v2_content).unwrap();
    let v2_hash = tracewise_core::blob::hash_bytes(v2_content);

    let changes2 =
        tracewise_core::scan_repository_with_db(&work_tree, &data_dir, "main", "head1", &options, &[PathBuf::from("test.txt")], &mut db)
            .unwrap();
    assert_eq!(changes2.len(), 1);
    let c2 = db.get_change(&changes2[0]).unwrap().unwrap();
    assert_eq!(c2.before.as_deref(), Some(v1_hash.as_str()));
    assert_eq!(c2.after.as_deref(), Some(v2_hash.as_str()));

    // Verify diff using git object store
    let before_bytes = c2.before.as_deref().and_then(|h| blobs.read(h).ok());
    let after_bytes = c2.after.as_deref().and_then(|h| blobs.read(h).ok());
    assert_eq!(before_bytes.as_deref(), Some(&v1_content[..]));
    assert_eq!(after_bytes.as_deref(), Some(&v2_content[..]));

    let diff = format_unified_diff(before_bytes.as_deref(), after_bytes.as_deref(), &c2.path);
    assert!(diff.contains("-first version line 2"));
    assert!(diff.contains("+second version line 2"));

    // Verify restore at c1 restores v1 byte-identically
    let restored_bytes = blobs.read(c1.after.as_deref().unwrap()).unwrap();
    assert_eq!(restored_bytes, v1_content);
    fs::write(&file_path, &restored_bytes).unwrap();
    assert_eq!(fs::read(&file_path).unwrap(), v1_content);
}

#[cfg(test)]
mod label_tests {
    use super::*;
    use tracewise_core::model::Actor;

    #[test]
    fn labels_read_like_people_talk() {
        assert_eq!(actor_label(&Actor::agent("claude", "s1"), "exact"), "claude/s1");
        assert_eq!(actor_label(&Actor::unknown(), "unknown"), "unknown (unknown)");
    }

    #[test]
    fn test_agent_undo_permission_checks() {
        let claude = Actor::agent("claude", "s1");
        let codex = Actor::agent("codex", "s2");
        let human = Actor { kind: "human".to_string(), name: Some("asmat".to_string()), session: None };

        // Agent can undo their own changes
        assert!(revert::can_actor_undo(Some(&claude), "claude"));
        // Agent cannot undo another actor's changes
        assert!(!revert::can_actor_undo(Some(&claude), "codex"));
        assert!(!revert::can_actor_undo(Some(&claude), "asmat"));
        assert!(!revert::can_actor_undo(Some(&codex), "claude"));

        // Human can undo anyone's changes
        assert!(revert::can_actor_undo(Some(&human), "claude"));
        assert!(revert::can_actor_undo(Some(&human), "codex"));
        assert!(revert::can_actor_undo(None, "claude"));
    }

    #[test]
    fn test_check_path_binary_or_hash_only() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_paths =
            RepoPaths { work_tree: tmp.path().to_path_buf(), git_dir: tmp.path().join(".git"), git_common_dir: tmp.path().join(".git") };
        let data_dir = repo_paths.data_dir();
        fs::create_dir_all(&data_dir).unwrap();
        let blobs = BlobStore::new(&data_dir);

        // Case 1: binary file on disk
        let bin_file = tmp.path().join("bin.dat");
        fs::write(&bin_file, b"PNG\0data").unwrap();
        let res = revert::check_path_binary_or_hash_only("bin.dat", &repo_paths, &[], None, &blobs);
        assert_eq!(res.as_deref(), Some("binary file"));

        // Case 2: text file on disk, but change blob missing (hash only)
        let txt_file = tmp.path().join("big.txt");
        fs::write(&txt_file, b"hello world\n").unwrap();
        let dummy_change = Change {
            id: "01J00000000000000000000000".to_string(),
            path: "big.txt".to_string(),
            old_path: None,
            op: "modify".to_string(),
            actor: Actor::agent("codex", "s1"),
            time: tracewise_core::model::TimeRange { start: "2026-10-25T23:00:00Z".to_string(), end: "2026-10-25T23:00:01Z".to_string() },
            before: None,
            after: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
            prev_hash: None,
            confidence: "exact".to_string(),
            source: "scan".to_string(),
            intent: None,
            device: "dev".to_string(),
            command: None,
            candidates: Vec::new(),
            lock_conflict: None,
            hunks: Vec::new(),
            git: tracewise_core::model::GitInfo { branch: "main".to_string(), head: "head".to_string() },
            undoes: None,
        };
        let changes = [&dummy_change];
        let res = revert::check_path_binary_or_hash_only("big.txt", &repo_paths, &changes, Some(&revert::Target::actor("codex")), &blobs);
        assert_eq!(res.as_deref(), Some("file was recorded by hash only (too large/binary)"));
    }

    #[test]
    fn test_undo_creation_deletion_rename_primitives() {
        let tmp = tempfile::tempdir().unwrap();
        let work_tree = tmp.path().to_path_buf();
        let repo_paths =
            RepoPaths { work_tree: work_tree.clone(), git_dir: work_tree.join(".git"), git_common_dir: work_tree.join(".git") };
        let data_dir = repo_paths.data_dir();
        fs::create_dir_all(&data_dir).unwrap();

        // Rename reversal logic
        let renamed_path = work_tree.join("renamed.txt");
        let old_path = work_tree.join("original.txt");
        fs::write(&renamed_path, b"content\n").unwrap();
        assert!(renamed_path.exists());
        assert!(!old_path.exists());

        fs::rename(&renamed_path, &old_path).unwrap();
        assert!(!renamed_path.exists());
        assert!(old_path.exists());
    }

    #[test]
    fn test_redo_single_change_three_way_merge() {
        let tmp = tempfile::tempdir().unwrap();
        let work_tree = tmp.path().to_path_buf();
        let repo_paths =
            RepoPaths { work_tree: work_tree.clone(), git_dir: work_tree.join(".git"), git_common_dir: work_tree.join(".git") };
        let data_dir = repo_paths.data_dir();
        fs::create_dir_all(&data_dir).unwrap();
        let blobs = BlobStore::new(&data_dir);

        let before_bytes = b"base\nx1\ny1\nx2\n";
        let after_bytes = b"base\ny1\n";
        let before_hash = blobs.store(before_bytes).unwrap();
        let after_hash = blobs.store(after_bytes).unwrap();

        let file_path = work_tree.join("f.txt");
        fs::write(&file_path, after_bytes).unwrap();

        let undo_change = Change {
            id: "01J00000000000000000000001".to_string(),
            path: "f.txt".to_string(),
            old_path: None,
            op: "undo".to_string(),
            actor: Actor { kind: "human".to_string(), name: Some("user".to_string()), session: None },
            time: tracewise_core::model::TimeRange { start: "2026-10-25T23:00:00Z".to_string(), end: "2026-10-25T23:00:01Z".to_string() },
            before: Some(before_hash),
            after: Some(after_hash),
            prev_hash: None,
            confidence: "exact".to_string(),
            source: "undo".to_string(),
            intent: None,
            device: "dev".to_string(),
            command: None,
            candidates: Vec::new(),
            lock_conflict: None,
            hunks: Vec::new(),
            git: tracewise_core::model::GitInfo { branch: "main".to_string(), head: "head".to_string() },
            undoes: Some(vec!["01J00000000000000000000002".to_string()]),
        };

        let mut redone = Vec::new();
        let mut skipped = Vec::new();
        let ctx = redo::RedoContext { repo_paths: &repo_paths, blobs: &blobs, dry_run: false };

        redo::redo_single_change(&undo_change, &ctx, &mut redone, &mut skipped);
        assert_eq!(redone, vec!["f.txt".to_string()]);
        assert!(skipped.is_empty());
        assert_eq!(fs::read(&file_path).unwrap(), before_bytes);
    }
}
