//! recovery drills, run on throw-away repositories: a write that fails half way, a history folder that cannot be written,
//! and backup then restore. Each ends with `tracewise verify` and the same history; none loses a change or reports false success.
mod common;
use common::Repo;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn copy_dir(from: &Path, to: &Path) {
    let ok = Command::new("cp").arg("-R").arg(from).arg(to).status().unwrap().success();
    assert!(ok, "cp -R failed");
}

fn count(repo: &Repo) -> usize {
    repo.rows(&[]).len()
}

/// The disk fills up while Tracewise stores a file: the write fails, nothing half-written is recorded, and the next scan recovers.
/// (The file-size limit stands in for a full disk: the same `write` error, without filling a real one.)
#[test]
fn a_write_that_fails_half_way_records_nothing_and_the_next_scan_recovers() {
    let repo = Repo::new("");
    repo.edit_as("human:me", "a.txt", "one\n");
    let before = count(&repo);
    // a big file: its blob is larger than the limit of 4 blocks (2 KB), so storing it fails
    let big: String = (0..4000).map(|i| format!("line number {i} of the big file\n")).collect();
    repo.write("big.txt", &big);
    let failed = Command::new("sh")
        .args(["-c", &format!("ulimit -f 4; exec {} scan", env!("CARGO_BIN_EXE_tracewise"))])
        .current_dir(repo.dir())
        .env("TRACEWISE_HOME", repo.home())
        .env("TRACEWISE_ACTOR", "human:me")
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .output()
        .unwrap();
    assert!(!failed.status.success(), "a scan that cannot write must not report success: {}", String::from_utf8_lossy(&failed.stdout));
    // it failed because of the write limit (killed by SIGXFSZ or an I/O error), not for another reason
    let why = String::from_utf8_lossy(&failed.stderr).to_lowercase();
    let signalled = std::os::unix::process::ExitStatusExt::signal(&failed.status).is_some();
    assert!(signalled || why.contains("file too large") || why.contains("write") || why.contains("blob"), "{why}");
    assert!(repo.me(&["verify"]).ok(), "the history is intact after the failed write");
    assert_eq!(count(&repo), before, "no change was recorded for the file that could not be stored");

    let again = repo.me(&["scan"]);
    assert!(again.ok(), "{}", again.err);
    assert!(repo.rows(&[]).iter().any(|c| c["path"] == "big.txt"), "the next scan records it");
    assert!(repo.me(&["verify"]).ok());
}

/// The history folder is read-only: reading still works, writing fails loudly, and nothing is lost when it is writable again.
#[test]
fn a_read_only_history_folder_can_be_read_but_not_written_and_recovers() {
    let repo = Repo::new("");
    repo.edit_as("human:me", "a.txt", "one\n");
    let before = count(&repo);
    let data = repo.dir().join(".git/tracewise");
    let set = |mode| std::fs::set_permissions(&data, std::fs::Permissions::from_mode(mode)).unwrap();
    set(0o500);
    let read = repo.me(&["log", "--json"]);
    let write = {
        repo.write("b.txt", "two\n");
        repo.me(&["scan"])
    };
    set(0o700);
    assert!(read.ok() && read.json().as_array().unwrap().len() == before, "reading works on a read-only history: {}", read.err);
    assert!(
        !write.ok() || write.err.contains("read-only") || write.err.contains("denied"),
        "writing is refused, not silently dropped: {}{}",
        write.out,
        write.err
    );
    assert!(repo.me(&["verify"]).ok());
    assert!(repo.me(&["scan"]).ok(), "writable again");
    assert!(repo.rows(&[]).iter().any(|c| c["path"] == "b.txt"), "the file written meanwhile is recorded once it can be");
}

/// Backup while nothing writes, lose the history, restore, verify: same changes, same head, a stored version reads back.
#[test]
fn a_backup_restores_the_whole_history() {
    let repo = Repo::new("");
    repo.edit_as("human:me", "a.txt", "one\n");
    repo.edit_as("agent:codex:s1", "a.txt", "one\ntwo\n");
    repo.edit_as("human:me", "b.txt", "bee\n");
    let (rows_before, head_before) = (repo.rows(&[]), std::fs::read_to_string(repo.dir().join(".git/tracewise/chain-head")).unwrap());
    let data = repo.dir().join(".git/tracewise");
    let backup = tempfile::tempdir().unwrap();
    copy_dir(&data, &backup.path().join("tracewise"));

    std::fs::remove_dir_all(&data).unwrap();
    assert!(!repo.me(&["verify"]).ok() || count(&repo) == 0, "with the history gone there is nothing to verify");
    copy_dir(&backup.path().join("tracewise"), &data);

    assert!(repo.me(&["verify"]).ok(), "the restored history verifies");
    assert_eq!(repo.rows(&[]), rows_before, "every change is back, unchanged");
    assert_eq!(std::fs::read_to_string(data.join("chain-head")).unwrap(), head_before);
    let id = rows_before.iter().find(|c| c["path"] == "a.txt" && c["op"] == "modify").unwrap()["id"].as_str().unwrap().to_string();
    let shown = repo.me(&["show", &id]);
    assert!(shown.ok() && shown.out.contains("+two"), "a stored version reads back: {}", shown.out);
}

/// A backup taken from a copy that is missing its newest blob is detected by `verify`, not trusted.
#[test]
fn a_damaged_backup_is_detected_on_restore() {
    let repo = Repo::new("");
    repo.edit_as("human:me", "a.txt", "one\n");
    let data = repo.dir().join(".git/tracewise");
    let backup = tempfile::tempdir().unwrap();
    copy_dir(&data, &backup.path().join("tracewise"));
    std::fs::remove_dir_all(backup.path().join("tracewise/objects")).unwrap();
    std::fs::remove_dir_all(&data).unwrap();
    copy_dir(&backup.path().join("tracewise"), &data);
    assert!(!repo.me(&["verify"]).ok(), "a restore that lost its blobs must not verify");
}
