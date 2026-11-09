use super::*;
use tempfile::TempDir;

fn batch(root: &Path, device: &str) -> PathBuf {
    let d = root.join("batches").join("b1");
    fs::create_dir_all(d.join("blobs")).unwrap();
    let meta = super::super::BatchMeta {
        id: "b1".into(),
        device: device.into(),
        from: "a".into(),
        to: "b".into(),
        format: 1,
        level: "full".into(),
    };
    fs::write(d.join("batch.json"), serde_json::to_string(&meta).unwrap()).unwrap();
    fs::write(d.join("changes.jsonl.zst"), b"records").unwrap();
    fs::write(d.join("blobs").join("x"), b"blob").unwrap();
    d
}

// one test: TRAIL_HOME is process-wide
#[test]
fn sign_verify_tamper_and_key_change() {
    let home = TempDir::new().unwrap();
    std::env::set_var("TRAIL_HOME", home.path());
    let other = TempDir::new().unwrap(); // a second machine's data folder: knows no pins
    let data = TempDir::new().unwrap();
    let d = batch(data.path(), "dev1");
    sign_batch(data.path(), &d, "dev1").unwrap();
    assert!(key_path().starts_with(home.path()));
    verify_batch(data.path(), &d).unwrap();

    // the other machine pins the key on first sight, then accepts the batch again
    let d2 = batch(other.path(), "dev1");
    fs::copy(d.join("signature.json"), d2.join("signature.json")).unwrap();
    verify_batch(other.path(), &d2).unwrap();
    assert_eq!(pinned(other.path()).len(), 1);
    verify_batch(other.path(), &d2).unwrap();

    // altered content, altered blob, stripped signature
    fs::write(d2.join("blobs").join("x"), b"evil").unwrap();
    assert!(verify_batch(other.path(), &d2).unwrap_err().to_string().contains("signature"));
    fs::write(d2.join("blobs").join("x"), b"blob").unwrap();
    fs::remove_file(d2.join("signature.json")).unwrap();
    assert!(verify_batch(other.path(), &d2).unwrap_err().to_string().contains("unsigned"));

    // a new key for a pinned device is refused until the pin is forgotten
    fs::remove_file(key_path()).unwrap();
    let d3 = batch(other.path(), "dev1");
    sign_batch(data.path(), &d3, "dev2").unwrap(); // signed with the new key; `data` pins it as dev2
    let err = verify_batch(other.path(), &d3).unwrap_err().to_string();
    assert!(err.contains("different key"), "{err}");
    assert!(forget(other.path(), "dev1").unwrap());
    verify_batch(other.path(), &d3).unwrap();

    // a batch from before signing is accepted for a device that never signed
    let (a, b) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    verify_batch(b.path(), &batch(a.path(), "olddev")).unwrap();
}
