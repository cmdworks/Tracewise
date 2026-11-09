//! Pushing unpushed changes to a companion git repository.
use super::batch::{ensure_companion_repo, git_cmd, git_rev_parse, run_git, BatchMeta};
use super::config::{contains_secret, is_remote_trusted, is_secret_path, read_trailconfig};
use crate::blob::BlobStore;
use crate::db::Database;
use crate::model::{Change, CommandRecord, Intent};
use std::fs;
use std::io;
use std::path::Path;

pub fn push(repo_root: &Path, data_dir: &Path) -> io::Result<Option<(String, usize)>> {
    let cfg = match read_trailconfig(repo_root) {
        Some(c) => c,
        None => return Ok(None),
    };
    if cfg.level == "off" {
        return Ok(None);
    }
    if !is_remote_trusted(data_dir, &cfg.remote) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "remote '{}' comes from .trailconfig and is not approved on this machine; check it, then run `trail share trust`",
                cfg.remote
            ),
        ));
    }

    let db_path = data_dir.join("trail.db");
    let db = Database::open(&db_path).map_err(|e| io::Error::other(e.to_string()))?;
    let unpushed = db.get_unpushed_changes().map_err(|e| io::Error::other(e.to_string()))?;
    if unpushed.is_empty() {
        return Ok(None);
    }

    let batch_id = ulid::Ulid::new().to_string();
    let device_id = crate::identity::device_for(data_dir);
    let from_time = unpushed.first().map(|c| c.time.start.clone()).unwrap_or_default();
    let to_time = unpushed.last().map(|c| c.time.end.clone()).unwrap_or_default();

    let meta =
        BatchMeta { id: batch_id.clone(), device: device_id.clone(), from: from_time, to: to_time, format: 1, level: cfg.level.clone() };

    let batch_dir = data_dir.join("batches").join(&batch_id);
    fs::create_dir_all(&batch_dir)?;

    write_batch_metadata(&batch_dir, &meta)?;
    write_batch_changes(&batch_dir, &unpushed)?;
    write_batch_intents(&batch_dir, &unpushed, &db)?;
    write_batch_commands(&batch_dir, &unpushed, &db)?;
    write_batch_blobs(&batch_dir, &unpushed, data_dir, &cfg.level)?;

    if cfg.encrypt {
        let key_id = super::crypt::encrypt_batch(data_dir, &batch_dir)?;
        let mut meta_value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(batch_dir.join("batch.json"))?).map_err(|e| io::Error::other(e.to_string()))?;
        meta_value["encrypted"] = true.into();
        meta_value["key_id"] = key_id.into();
        fs::write(batch_dir.join("batch.json"), serde_json::to_string_pretty(&meta_value).unwrap_or_default())?;
    }
    // signed last: the signature covers the encrypted bytes
    super::sign::sign_batch(data_dir, &batch_dir, &device_id)?;

    let remote_dir = data_dir.join("remote");
    ensure_companion_repo(&cfg.remote, &remote_dir)?;
    let prefix = if cfg.hidden_refs { "refs/trail/device/" } else { "refs/heads/trail/device/" };
    let branch_ref = format!("{prefix}{device_id}");
    // the companion repository holds no copy of the remote: learn this device's own previous batches first, so the new one has a parent
    let _ = run_git(&remote_dir, &["fetch", "origin", &format!("+{branch_ref}:{branch_ref}")]);
    commit_and_push_batch(&remote_dir, &batch_dir, &branch_ref, &batch_id)?;

    let ids: Vec<String> = unpushed.into_iter().map(|c| c.id).collect();
    let count = ids.len();
    db.mark_changes_pushed(&ids).map_err(|e| io::Error::other(e.to_string()))?;
    // its records are already in this database: a later fetch need not open the batch
    let _ = db.mark_batch_imported(&batch_id);

    Ok(Some((batch_id, count)))
}

fn write_batch_metadata(batch_dir: &Path, meta: &BatchMeta) -> io::Result<()> {
    let meta_json = serde_json::to_string_pretty(meta).map_err(|e| io::Error::other(e.to_string()))?;
    fs::write(batch_dir.join("batch.json"), meta_json)
}

fn write_batch_changes(batch_dir: &Path, unpushed: &[Change]) -> io::Result<()> {
    let mut changes_lines = Vec::new();
    let mut secret_detected = false;
    for c in unpushed {
        if is_secret_path(&c.path) {
            secret_detected = true;
            let mut redacted = c.clone();
            redacted.hunks.clear();
            redacted.before = None;
            redacted.after = None;
            let line = serde_json::to_string(&redacted).map_err(|e| io::Error::other(e.to_string()))?;
            changes_lines.push(line);
        } else {
            let line = serde_json::to_string(c).map_err(|e| io::Error::other(e.to_string()))?;
            changes_lines.push(line);
        }
    }
    if secret_detected {
        eprintln!("trail: warning: secret files (.env*, *.pem, id_*) redacted from shared companion repository");
    }
    let changes_raw = changes_lines.join("\n") + "\n";
    let changes_zst = zstd::encode_all(changes_raw.as_bytes(), 3)?;
    fs::write(batch_dir.join("changes.jsonl.zst"), changes_zst)
}

fn write_batch_intents(batch_dir: &Path, unpushed: &[Change], db: &Database) -> io::Result<()> {
    let mut intents = Vec::new();
    for c in unpushed {
        if let Some(ref iid) = c.intent {
            if let Ok(Some(intent)) = db.get_intent(iid) {
                if !intents.iter().any(|i: &Intent| i.id == intent.id) {
                    intents.push(intent);
                }
            }
        }
    }
    if let Ok(all) = db.list_intents(None) {
        for i in all {
            if !intents.iter().any(|x| x.id == i.id) {
                intents.push(i);
            }
        }
    }
    let mut intents_lines = Vec::new();
    for i in &intents {
        let line = serde_json::to_string(i).map_err(|e| io::Error::other(e.to_string()))?;
        intents_lines.push(line);
    }
    let intents_raw = if intents_lines.is_empty() { String::new() } else { intents_lines.join("\n") + "\n" };
    let intents_zst = zstd::encode_all(intents_raw.as_bytes(), 3)?;
    fs::write(batch_dir.join("intents.jsonl.zst"), intents_zst)
}

fn write_batch_commands(batch_dir: &Path, unpushed: &[Change], db: &Database) -> io::Result<()> {
    let mut commands = Vec::new();
    for c in unpushed {
        if let Some(ref cid) = c.command {
            if let Ok(cmds) = db.list_commands() {
                if let Some(cmd) = cmds.into_iter().find(|cmd| &cmd.id == cid) {
                    if !commands.iter().any(|cm: &CommandRecord| cm.id == cmd.id) {
                        commands.push(cmd);
                    }
                }
            }
        }
    }
    let mut cmd_lines = Vec::new();
    for cmd in &commands {
        let line = serde_json::to_string(cmd).map_err(|e| io::Error::other(e.to_string()))?;
        cmd_lines.push(line);
    }
    let cmd_raw = if cmd_lines.is_empty() { String::new() } else { cmd_lines.join("\n") + "\n" };
    let cmd_zst = zstd::encode_all(cmd_raw.as_bytes(), 3)?;
    fs::write(batch_dir.join("commands.jsonl.zst"), cmd_zst)?;

    let anchors_zst = zstd::encode_all(&b""[..], 3)?;
    fs::write(batch_dir.join("anchors.jsonl.zst"), anchors_zst)
}

fn write_batch_blobs(batch_dir: &Path, unpushed: &[Change], data_dir: &Path, level: &str) -> io::Result<()> {
    let blob_store = BlobStore::new(data_dir);
    if level == "full" {
        let blobs_dir = batch_dir.join("blobs");
        fs::create_dir_all(&blobs_dir)?;
        for c in unpushed {
            if is_secret_path(&c.path) {
                continue;
            }
            for h in [&c.before, &c.after].into_iter().flatten() {
                match blob_store.read(h) {
                    Ok(bytes) => {
                        if !contains_secret(&bytes) {
                            fs::write(blobs_dir.join(h), bytes)?;
                        }
                    }
                    // a change with line hunks had its text stored: not being able to read it is damage, not a choice
                    Err(e) if !c.hunks.is_empty() => {
                        return Err(io::Error::other(format!("cannot push change {}: its stored version {h} is unreadable: {e}", c.id)));
                    }
                    // big and binary files are kept as hashes only
                    Err(_) => {}
                }
            }
        }
    }
    Ok(())
}

fn commit_and_push_batch(remote_dir: &Path, batch_dir: &Path, branch_ref: &str, batch_id: &str) -> io::Result<()> {
    let git_dir_arg = format!("--git-dir={}", remote_dir.display());
    let work_tree_arg = format!("--work-tree={}", batch_dir.display());

    let _ = fs::remove_file(remote_dir.join("index"));

    let add_out = git_cmd().args([&git_dir_arg, &work_tree_arg, "add", "-A"]).output()?;
    if !add_out.status.success() {
        let stderr = String::from_utf8_lossy(&add_out.stderr);
        return Err(io::Error::other(format!("git add failed: {}", stderr.trim())));
    }

    let write_tree_out = run_git(remote_dir, &["write-tree"])?;
    let tree_sha = String::from_utf8_lossy(&write_tree_out.stdout).trim().to_string();

    let parent_sha = git_rev_parse(remote_dir, branch_ref);

    let mut commit_cmd = git_cmd();
    commit_cmd.args([&git_dir_arg, "commit-tree", &tree_sha]);
    if let Some(ref p) = parent_sha {
        commit_cmd.args(["-p", p]);
    }
    commit_cmd.args(["-m", &format!("batch {}", batch_id)]);
    commit_cmd
        .env("GIT_AUTHOR_NAME", "trail")
        .env("GIT_AUTHOR_EMAIL", "trail@local")
        .env("GIT_COMMITTER_NAME", "trail")
        .env("GIT_COMMITTER_EMAIL", "trail@local");
    let commit_out = commit_cmd.output()?;
    if !commit_out.status.success() {
        let stderr = String::from_utf8_lossy(&commit_out.stderr);
        return Err(io::Error::other(format!("git commit-tree failed: {}", stderr.trim())));
    }
    let commit_sha = String::from_utf8_lossy(&commit_out.stdout).trim().to_string();

    run_git(remote_dir, &["update-ref", branch_ref, &commit_sha])?;

    let push_ref_spec = format!("{}:{}", branch_ref, branch_ref);
    run_git(remote_dir, &["push", "origin", &push_ref_spec])?;
    Ok(())
}

/// Deletes this device's ref on the remote and in the companion copy, so the next push starts a fresh history for the device
/// (`trail share key rekey --prune-old`). Batches already fetched by others stay in their data folders; the server may keep old objects.
pub fn delete_own_ref(repo_root: &Path, data_dir: &Path) -> io::Result<()> {
    let cfg = read_trailconfig(repo_root).ok_or_else(|| io::Error::other("sharing is not set up (.trailconfig)"))?;
    let prefix = if cfg.hidden_refs { "refs/trail/device/" } else { "refs/heads/trail/device/" };
    let branch_ref = format!("{prefix}{}", crate::identity::device_for(data_dir));
    let remote_dir = data_dir.join("remote");
    ensure_companion_repo(&cfg.remote, &remote_dir)?;
    run_git(&remote_dir, &["push", "origin", &format!(":{branch_ref}")])?;
    let _ = run_git(&remote_dir, &["update-ref", "-d", &branch_ref]);
    Ok(())
}
