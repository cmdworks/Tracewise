use super::*;
use tempfile::TempDir;

fn batch(root: &Path) -> PathBuf {
    let d = root.join("b");
    fs::create_dir_all(d.join("blobs")).unwrap();
    fs::write(d.join("batch.json"), b"{\"id\":\"b\"}").unwrap();
    fs::write(d.join("changes.jsonl.zst"), b"secret records").unwrap();
    fs::write(d.join("blobs").join("aaaa1111"), b"secret file contents").unwrap();
    d
}

#[test]
fn a_batch_round_trips_and_hides_its_contents_and_blob_names() {
    let (data, work) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let d = batch(work.path());
    assert!(encrypt_batch(data.path(), &d).unwrap_err().to_string().contains("has no key"));
    ensure_key(data.path()).unwrap();
    let id = encrypt_batch(data.path(), &d).unwrap();
    assert_eq!(fs::read(d.join("batch.json")).unwrap(), b"{\"id\":\"b\"}", "batch.json stays readable");
    assert!(!d.join("blobs").join("aaaa1111").exists(), "the plain-content hash is not kept as a name");
    for f in files_of(&d).unwrap() {
        if !f.ends_with("batch.json") {
            let bytes = fs::read(&f).unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("secret"), "{f:?} still holds plain text");
        }
    }
    let out = work.path().join("plain");
    decrypt_batch(data.path(), &d, &id, &out).unwrap();
    assert_eq!(fs::read(out.join("changes.jsonl.zst")).unwrap(), b"secret records");
    let blob = files_of(&out.join("blobs")).unwrap();
    assert_eq!((blob.len(), fs::read(&blob[0]).unwrap()), (1, b"secret file contents".to_vec()));
}

#[test]
fn a_wrong_key_a_missing_key_and_a_changed_file_are_refused() {
    let (data, other, work) = (TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap());
    let d = batch(work.path());
    ensure_key(data.path()).unwrap();
    let id = encrypt_batch(data.path(), &d).unwrap();
    let out = work.path().join("plain");
    let missing = decrypt_batch(other.path(), &d, &id, &out).unwrap_err().to_string();
    assert!(missing.contains("tracewise share key import"), "{missing}");
    add_new_key(other.path()).unwrap();
    let ring = load(other.path());
    // the other machine knows a key with a different id: still refused as missing; with the id forced it fails to decrypt
    assert!(decrypt_batch(other.path(), &d, &id, &out).is_err());
    let mut forged = ring.clone();
    forged.keys[0].0 = id.clone();
    save(other.path(), &forged).unwrap();
    assert!(decrypt_batch(other.path(), &d, &id, &out).unwrap_err().to_string().contains("wrong or the file was changed"));
    // a flipped byte
    let mut bytes = fs::read(d.join("changes.jsonl.zst")).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(d.join("changes.jsonl.zst"), bytes).unwrap();
    assert!(decrypt_batch(data.path(), &d, &id, &out).is_err());
}

#[test]
fn keys_are_made_once_and_rotation_keeps_the_old_one() {
    let data = TempDir::new().unwrap();
    let first = ensure_key(data.path()).unwrap();
    assert_eq!(ensure_key(data.path()).unwrap(), first, "a second call keeps the key");
    let second = add_new_key(data.path()).unwrap();
    let ring = load(data.path());
    assert_ne!(first, second);
    assert_eq!((ring.keys.len(), ring.current.as_deref()), (2, Some(second.as_str())));
    assert!(ring.keys.iter().any(|(id, _)| *id == first), "the old key still opens old batches");
}

#[test]
fn an_exported_key_imports_elsewhere_and_a_forged_one_is_refused() {
    let (a, b) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let id = ensure_key(a.path()).unwrap();
    let text = export_key(a.path(), None).unwrap();
    assert!(text.starts_with("tracewisekey1:") && text.contains(&id));
    assert_eq!(import_key(b.path(), &text, false).unwrap(), id);
    assert_eq!(load(b.path()).current.as_deref(), Some(id.as_str()), "the first key imported becomes current");
    // a later import does not take over unless asked
    let second = add_new_key(a.path()).unwrap();
    let t2 = export_key(a.path(), None).unwrap();
    import_key(b.path(), &t2, false).unwrap();
    assert_eq!(load(b.path()).current.as_deref(), Some(id.as_str()));
    import_key(b.path(), &t2, true).unwrap();
    assert_eq!((load(b.path()).current.as_deref(), load(b.path()).keys.len()), (Some(second.as_str()), 2));
    assert!(import_key(b.path(), "tracewisekey1:0000000000000000:".to_string().as_str(), false).is_err());
    let forged = text.replace(&id, "ffffffffffffffff");
    assert!(import_key(b.path(), &forged, false).is_err());
    assert!(export_key(TempDir::new().unwrap().path(), None).is_err());
}
