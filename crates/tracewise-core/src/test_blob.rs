use super::*;

#[test]
fn a_packed_project_blob_is_readable_from_any_directory() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("/usr/bin/git")
            .args(args)
            .current_dir(&repo)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(out.status.success());
    };
    git(&["init", "-q"]);
    fs::write(repo.join("a.txt"), b"in a pack\n").unwrap();
    git(&["add", "."]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "x"]);
    git(&["repack", "-adq"]);
    let store = BlobStore::new(&repo.join(".git/tracewise"));
    let hash = hash_bytes(b"in a pack\n");
    // stored as already present (no copy) ...
    assert_eq!(store.store(b"in a pack\n").unwrap(), hash);
    assert!(!store.root().join(&hash[..2]).exists());
    // ... and still readable by git when the current directory is no repository
    let mut cmd = store.git_on_objects();
    let out = cmd.args(["cat-file", "-p", &hash]).current_dir(temp.path()).output().unwrap();
    assert_eq!(out.stdout, b"in a pack\n");
}

#[test]
fn packed_objects_are_found_without_starting_git() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("/usr/bin/git")
            .args(args)
            .current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    let pack_dir = repo.join(".git/objects/pack");
    // a fresh repository has an empty pack folder: nothing is in it
    assert_eq!(pack_indexes_contain(&pack_dir, &hash_bytes(b"packed\n")), Some(false));
    fs::write(repo.join("a.txt"), b"packed\n").unwrap();
    fs::write(repo.join("b.txt"), b"other\n").unwrap();
    git(&["add", "."]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "x"]);
    git(&["repack", "-adq"]);
    for name in ["a.txt", "b.txt"] {
        let data = fs::read(repo.join(name)).unwrap();
        assert_eq!(pack_indexes_contain(&pack_dir, &hash_bytes(&data)), Some(true), "{name}");
    }
    assert_eq!(pack_indexes_contain(&pack_dir, &hash_bytes(b"not stored\n")), Some(false));
    assert_eq!(pack_indexes_contain(&pack_dir, "zz"), None);
}

#[test]
fn test_hash_bytes() {
    let data = b"hello\n";
    let h = hash_bytes(data);
    assert_eq!(h, "ce013625030ba8dba906f756967f9e9ca394464a");
    assert_eq!(h.len(), 40);
}

#[test]
fn test_blob_store_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let store = BlobStore::new(temp.path());
    let content = b"hello tracewise blob store\n";
    let hash = store.store(content).unwrap();
    assert_eq!(hash, hash_bytes(content));
    let read_back = store.read(&hash).unwrap();
    assert_eq!(read_back, content);
}

#[test]
fn test_binary_or_large() {
    assert!(is_binary_or_large(b"hello\0world"));
    assert!(!is_binary_or_large(b"hello world\n"));
}

#[test]
fn test_blob_store_alternates_dedupe() {
    let temp = tempfile::tempdir().unwrap();
    let git_objects = temp.path().join("git_objects");
    let tracewise_objects = temp.path().join("tracewise_objects");

    let store = BlobStore::with_git_objects(tracewise_objects.clone(), Some(git_objects.clone()));

    // Pre-create object in git_objects
    let content = b"committed to project git\n";
    let hash = hash_bytes(content);
    let (prefix, rest) = hash.split_at(2);
    let git_loose_dir = git_objects.join(prefix);
    fs::create_dir_all(&git_loose_dir).unwrap();

    // Write loose object in git_objects
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(format!("blob {}\0", content.len()).as_bytes()).unwrap();
    encoder.write_all(content).unwrap();
    let compressed = encoder.finish().unwrap();
    fs::write(git_loose_dir.join(rest), compressed).unwrap();

    // Now store through BlobStore
    let stored_hash = store.store(content).unwrap();
    assert_eq!(stored_hash, hash);

    // Verify it was NOT written into tracewise_objects loose
    let tracewise_loose = tracewise_objects.join(prefix).join(rest);
    assert!(!tracewise_loose.exists());

    // But read works through alternate
    let read_back = store.read(&hash).unwrap();
    assert_eq!(read_back, content);

    // Verify alternates file exists
    let alt_file = tracewise_objects.join("info").join("alternates");
    assert!(alt_file.is_file());
    let alt_content = fs::read_to_string(alt_file).unwrap();
    assert!(alt_content.contains(git_objects.to_str().unwrap()));
}

#[test]
fn test_blob_store_gc() {
    let temp = tempfile::tempdir().unwrap();
    let store = BlobStore::new(temp.path());

    // Store multiple versions of a non-trivial document so delta compression dominates index overhead
    let mut hashes = Vec::new();
    let base = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(200);
    for i in 0..20 {
        let data = format!("{base}\nVersion variation {i} with some unique content\n");
        let h = store.store(data.as_bytes()).unwrap();
        hashes.push((h, data));
    }

    let report = store.gc().unwrap();
    assert_eq!(report.packed, 20);
    assert!(
        report.bytes_after <= report.bytes_before,
        "bytes_after ({}) should be <= bytes_before ({})",
        report.bytes_after,
        report.bytes_before
    );

    // Verify all versions still readable after gc
    for (h, expected) in hashes {
        let read_back = store.read(&h).unwrap();
        assert_eq!(read_back, expected.as_bytes());
    }
}

#[test]
fn the_durability_level_is_read_from_the_repository_config() {
    let repo = tempfile::tempdir().unwrap();
    let data = repo.path().join(".git").join("tracewise");
    std::fs::create_dir_all(&data).unwrap();
    assert!(load_durability(&data), "default is full");
    std::fs::write(repo.path().join(".tracewiseconfig"), "[durability]\nlevel = \"normal\"\n").unwrap();
    assert!(!load_durability(&data));
    std::fs::write(repo.path().join(".tracewiseconfig"), "[durability]\nlevel = \"full\"\n").unwrap();
    assert!(load_durability(&data));
    let store = BlobStore::new(&data);
    let hash = store.store(b"hello\n").unwrap();
    assert_eq!(store.read(&hash).unwrap(), b"hello\n", "a blob stored with fsync reads back");
}

#[test]
fn one_flush_covers_everything_marked_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let mark = std::sync::atomic::AtomicBool::new(false);
    assert!(!super::flush_marked(&mark, dir.path()).unwrap(), "nothing marked, nothing flushed");
    mark.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(super::flush_marked(&mark, dir.path()).unwrap(), "marked, so flushed");
    assert!(!super::flush_marked(&mark, dir.path()).unwrap(), "the mark is cleared: one flush per batch");
}
