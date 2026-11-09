//! Companion git repository operations and remote batch sync.
use super::config::{is_safe_id, is_safe_rel_path, validate_remote_url};
use crate::blob::BlobStore;
use crate::db::Database;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatchMeta {
    pub id: String,
    pub device: String,
    pub from: String,
    pub to: String,
    pub format: u32,
    pub level: String,
}

pub(crate) fn git_cmd() -> Command {
    let target = if let Ok(p) = std::env::var("TRAIL_REAL_GIT") {
        p
    } else if Path::new("/usr/bin/git").is_file() {
        "/usr/bin/git".to_string()
    } else {
        "git".to_string()
    };
    let mut cmd = Command::new(target);
    cmd.env("TRAIL_GIT_DEPTH", "1");
    cmd
}

pub(crate) fn run_git_bare(args: &[&str]) -> io::Result<std::process::Output> {
    let output = git_cmd().args(args).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("git {} failed: {}", args.join(" "), stderr.trim())));
    }
    Ok(output)
}

pub(crate) fn run_git(git_dir: &Path, args: &[&str]) -> io::Result<std::process::Output> {
    let git_dir_arg = format!("--git-dir={}", git_dir.display());
    let mut cmd = git_cmd();
    cmd.arg(&git_dir_arg).args(args);
    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("git {} failed: {}", args.join(" "), stderr.trim())));
    }
    Ok(output)
}

pub(crate) fn git_rev_parse(git_dir: &Path, ref_name: &str) -> Option<String> {
    let git_dir_arg = format!("--git-dir={}", git_dir.display());
    let out = git_cmd().args([&git_dir_arg, "rev-parse", "--verify", ref_name]).output().ok()?;
    if out.status.success() {
        let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !sha.is_empty() {
            return Some(sha);
        }
    }
    None
}

pub fn ensure_companion_repo(remote_url: &str, remote_dir: &Path) -> io::Result<()> {
    validate_remote_url(remote_url)?;
    if remote_dir.exists() {
        run_git(remote_dir, &["remote", "set-url", "origin", "--", remote_url])
            .or_else(|_| run_git(remote_dir, &["remote", "add", "origin", "--", remote_url]))
            .map_err(|e| io::Error::other(format!("could not point the companion repository at '{remote_url}': {e}")))?;
        return Ok(());
    }
    if let Some(parent) = remote_dir.parent() {
        fs::create_dir_all(parent)?;
    }

    // An empty bare repository that knows the remote, not a clone: `fetch` asks only for `trail/device/*` refs, so the companion
    // never copies the project's own branches when the remote is the project's repository.
    let dest_str = remote_dir.to_string_lossy();
    run_git_bare(&["init", "--bare", "-q", "--", &dest_str])
        .map_err(|e| io::Error::other(format!("failed to create the companion repository for '{remote_url}': {e}")))?;
    run_git(remote_dir, &["remote", "add", "origin", "--", remote_url])
        .map_err(|e| io::Error::other(format!("could not point the companion repository at '{remote_url}': {e}")))?;
    // an unreachable remote fails here, as the clone used to, and leaves no half-made folder behind
    if let Err(e) = run_git(remote_dir, &["ls-remote", "origin"]) {
        let _ = fs::remove_dir_all(remote_dir);
        return Err(io::Error::other(format!("failed to reach companion repository '{remote_url}': {e}")));
    }
    Ok(())
}

pub(crate) fn copy_remote_batches(remote_dir: &Path, data_dir: &Path) -> io::Result<()> {
    let refs_out = run_git(remote_dir, &["for-each-ref", "--format=%(refname)", "refs/heads/trail/device/", "refs/trail/device/"])?;
    for branch_ref in String::from_utf8_lossy(&refs_out.stdout).lines() {
        let branch_ref = branch_ref.trim();
        if branch_ref.is_empty() {
            continue;
        }
        let rev_out = run_git(remote_dir, &["rev-list", "--reverse", branch_ref])?;
        for commit in String::from_utf8_lossy(&rev_out.stdout).lines() {
            let commit = commit.trim();
            if commit.is_empty() {
                continue;
            }
            let Ok(show_out) = run_git(remote_dir, &["show", &format!("{commit}:batch.json")]) else {
                continue;
            };
            let Ok(meta) = serde_json::from_slice::<BatchMeta>(&show_out.stdout) else {
                continue;
            };
            if !is_safe_id(&meta.id) {
                continue;
            }
            let batch_dir = data_dir.join("batches").join(&meta.id);
            if batch_dir.exists() {
                continue;
            }
            // staged next to the final folder and renamed when complete: a failure half way leaves nothing that a retry would skip
            let staging = data_dir.join("batches").join(format!(".incoming-{}", meta.id));
            let _ = fs::remove_dir_all(&staging);
            fs::create_dir_all(&staging)?;
            if let Err(e) = copy_commit_files(remote_dir, commit, &staging) {
                let _ = fs::remove_dir_all(&staging);
                return Err(e);
            }
            fs::rename(&staging, &batch_dir)?;
        }
    }
    Ok(())
}

/// Writes the regular files of `commit`'s tree into `dest_root`; unsafe paths are skipped on purpose, every other failure is an error.
fn copy_commit_files(remote_dir: &Path, commit: &str, dest_root: &Path) -> io::Result<()> {
    let tree_out = run_git(remote_dir, &["ls-tree", "-r", "-l", commit])?; // -l adds each file's size
    for entry in String::from_utf8_lossy(&tree_out.stdout).lines() {
        let Some((info, file_path)) = entry.split_once('\t') else { continue };
        let parts: Vec<&str> = info.split_whitespace().collect();
        if parts.len() < 4 || !matches!(parts[0], "100644" | "100755") || !is_safe_rel_path(file_path) {
            continue;
        }
        if parts[3].parse::<u64>().map_or(true, |n| n > super::fetch::MAX_BATCH_BYTES) {
            return Err(io::Error::other(format!("{file_path} in the remote batch is over 50 MB")));
        }
        let cat_out = run_git(remote_dir, &["cat-file", "-p", parts[2]])?;
        let dest = dest_root.join(file_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(dest, cat_out.stdout)?;
    }
    Ok(())
}

/// Imports every local batch in one transaction: all of them, or (on any error) none. `respect_notes` skips batches the database already
/// noted as imported (`fetch`); `rebuild` reads every batch.
pub(crate) fn import_all_batches(data_dir: &Path, repo_root: &Path, respect_notes: bool) -> io::Result<usize> {
    let mut db = Database::open(&data_dir.join("trail.db")).map_err(|e| io::Error::other(e.to_string()))?;
    db.begin_batch().map_err(|e| io::Error::other(e.to_string()))?;
    match import_all_batches_with(&mut db, data_dir, repo_root, respect_notes) {
        Ok(n) => db.commit_batch().map(|_| n).map_err(|e| io::Error::other(e.to_string())),
        Err(e) => {
            let _ = db.rollback_batch();
            Err(e)
        }
    }
}

/// The caller owns the transaction.
fn import_all_batches_with(db: &mut Database, data_dir: &Path, repo_root: &Path, respect_notes: bool) -> io::Result<usize> {
    let blob_store = BlobStore::new(data_dir);
    let meta_of = |dir: &Path| fs::read_to_string(dir.join("batch.json")).ok().and_then(|s| serde_json::from_str::<BatchMeta>(&s).ok());
    let mut batch_dirs: Vec<_> = fs::read_dir(data_dir.join("batches"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    batch_dirs.sort_by(|a, b| match (meta_of(a), meta_of(b)) {
        (Some(ma), Some(mb)) => ma.from.cmp(&mb.from),
        _ => a.cmp(b),
    });
    let mut total = 0;
    for dir in batch_dirs {
        let batch_id = dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if respect_notes && db.batch_imported(&batch_id) {
            continue;
        }
        super::sign::verify_batch(data_dir, &dir)?;
        // an encrypted batch is decrypted into a scratch folder (removed afterwards) and imported from there
        let encrypted = fs::read_to_string(dir.join("batch.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .filter(|v| v["encrypted"] == true)
            .and_then(|v| v["key_id"].as_str().map(String::from));
        match encrypted {
            Some(key_id) => {
                let plain =
                    data_dir.join("batches").join(format!(".decrypted-{}", dir.file_name().and_then(|n| n.to_str()).unwrap_or("b")));
                let res = super::crypt::decrypt_batch(data_dir, &dir, &key_id, &plain)
                    .and_then(|_| super::fetch::import_batch_dir(&plain, db, &blob_store, repo_root));
                let _ = fs::remove_dir_all(&plain);
                total += res?;
            }
            None => total += super::fetch::import_batch_dir(&dir, db, &blob_store, repo_root)?,
        }
        db.mark_batch_imported(&batch_id).map_err(|e| io::Error::other(e.to_string()))?;
    }
    Ok(total)
}
