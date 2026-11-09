use super::*;
use crate::model::Change;
use crate::share::batch::{self, copy_remote_batches};
use std::path::PathBuf;

fn write_zst(path: &Path, text: &str) {
    fs::write(path, zstd::encode_all(text.as_bytes(), 1).unwrap()).unwrap();
}

#[test]
fn a_batch_file_over_the_limits_is_refused_before_it_fills_memory() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("changes.jsonl.zst");

    // 50 MB of text that compresses to a few KB (a decompression bomb)
    write_zst(&p, &"\n".repeat(51 * 1024 * 1024));
    let err = read_records::<Change>(&p).unwrap_err().to_string();
    assert!(err.contains("50 MB"), "{err}");

    // too many records
    write_zst(&p, &"{}\n".repeat(100_001));
    assert!(read_records::<serde_json::Value>(&p).unwrap_err().to_string().contains("too many"));

    // one huge record
    write_zst(&p, &format!("\"{}\"\n", "x".repeat(2 * 1024 * 1024)));
    assert!(read_records::<serde_json::Value>(&p).unwrap_err().to_string().contains("1 MB"));

    // a normal file still reads, and a missing one is empty
    write_zst(&p, "{\"a\":1}\n{\"a\":2}\n");
    assert_eq!(read_records::<serde_json::Value>(&p).unwrap().len(), 2);
    assert!(read_records::<serde_json::Value>(&dir.path().join("none.zst")).unwrap().is_empty());
}

fn remote_with_batch(root: &Path, id: &str) -> PathBuf {
    let remote = root.join("remote.git");
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    fs::write(work.join("batch.json"), format!(r#"{{"id":"{id}","device":"d","from":"a","to":"b","format":1,"level":"x"}}"#)).unwrap();
    fs::write(work.join("changes.jsonl.zst"), b"data").unwrap();
    let git = |args: &[&str]| {
        let out = batch::git_cmd()
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .arg(format!("--git-dir={}", remote.display()))
            .arg(format!("--work-tree={}", work.display()))
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    batch::git_cmd().args(["init", "-q", "--bare", remote.to_str().unwrap()]).output().unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "b"]);
    git(&["branch", "-m", "tracewise/device/d"]);
    remote
}

#[test]
fn a_crash_leftover_staging_folder_is_replaced_and_never_imported() {
    let d = tempfile::tempdir().unwrap();
    let remote = remote_with_batch(d.path(), "B1");
    let data = d.path().join("data");
    let stale = data.join("batches/.incoming-B1");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join("junk"), b"half written").unwrap();

    copy_remote_batches(&remote, &data).unwrap();
    assert!(!stale.exists(), "the staging folder is gone after the rename");
    assert_eq!(fs::read(data.join("batches/B1/changes.jsonl.zst")).unwrap(), b"data");
    assert!(!data.join("batches/B1/junk").exists(), "nothing from the old half copy");

    // a second run skips a batch that is already complete
    copy_remote_batches(&remote, &data).unwrap();
    assert!(data.join("batches/B1/batch.json").is_file());
}
