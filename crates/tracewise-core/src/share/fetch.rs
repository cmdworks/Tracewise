//! Fetching batches from remote companion repositories, importing, and rebuilding.
use super::batch::{copy_remote_batches, ensure_companion_repo, import_all_batches, run_git};
use super::config::{is_safe_rel_path, may_sync_from, read_tracewiseconfig};
use crate::blob::BlobStore;
use crate::db::Database;
use crate::model::{Change, CommandRecord, FileRecord, Intent};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

#[cfg(test)]
#[path = "test_fetch.rs"]
mod tests;

fn db_err(e: rusqlite::Error) -> io::Error {
    io::Error::other(e.to_string())
}

/// Imports one batch; any failure is an error (the caller's transaction makes it all or nothing).
pub fn import_batch_dir(batch_dir: &Path, db: &mut Database, blob_store: &BlobStore, _repo_root: &Path) -> io::Result<usize> {
    import_blobs(batch_dir, blob_store)?;
    import_intents(batch_dir, db)?;
    import_commands(batch_dir, db)?;
    import_changes(batch_dir, db, blob_store)
}

fn import_blobs(batch_dir: &Path, blob_store: &BlobStore) -> io::Result<()> {
    let blobs_dir = batch_dir.join("blobs");
    if !blobs_dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&blobs_dir)?.flatten() {
        if entry.file_type()?.is_file() {
            if entry.metadata()?.len() > MAX_BATCH_BYTES {
                return Err(io::Error::other(format!("{}: blob over 50 MB", entry.path().display())));
            }
            blob_store.store(&fs::read(entry.path())?)?;
        }
    }
    Ok(())
}

/// Limits for one batch file from a remote: decompressed size, records, and bytes per record.
pub(crate) const MAX_BATCH_BYTES: u64 = 50 * 1024 * 1024;
const MAX_RECORDS: usize = 100_000;
const MAX_RECORD_BYTES: usize = 1024 * 1024;

/// The lines of a zstd JSON-lines file of a batch (missing file: no lines). A line that does not parse, or a file over
/// the limits above, is an error; the decompression stops at the limit instead of filling memory.
fn read_records<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Vec<T>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let mut text = String::new();
    zstd::stream::Decoder::new(fs::File::open(path)?)?.take(MAX_BATCH_BYTES + 1).read_to_string(&mut text)?;
    let bad = |why: &str| io::Error::other(format!("{}: {why}", path.display()));
    if text.len() as u64 > MAX_BATCH_BYTES {
        return Err(bad("more than 50 MB once decompressed"));
    }
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.len() > MAX_RECORDS || lines.iter().any(|l| l.len() > MAX_RECORD_BYTES) {
        return Err(bad("too many records or a record over 1 MB"));
    }
    lines.into_iter().map(|l| serde_json::from_str(l).map_err(|e| bad(&format!("bad record: {e}")))).collect()
}

fn import_intents(batch_dir: &Path, db: &Database) -> io::Result<()> {
    for intent in read_records::<Intent>(&batch_dir.join("intents.jsonl.zst"))? {
        db.insert_intent_imported(&intent).map_err(db_err)?;
    }
    Ok(())
}

fn import_commands(batch_dir: &Path, db: &Database) -> io::Result<()> {
    for cmd in read_records::<CommandRecord>(&batch_dir.join("commands.jsonl.zst"))? {
        db.insert_command_imported(&cmd).map_err(db_err)?;
        db.link_changes_to_command(&cmd.changes, &cmd.id).map_err(db_err)?;
    }
    Ok(())
}

fn import_changes(batch_dir: &Path, db: &mut Database, blob_store: &BlobStore) -> io::Result<usize> {
    let mut changes = read_records::<Change>(&batch_dir.join("changes.jsonl.zst"))?;
    changes.sort_by(|a, b| a.time.start.cmp(&b.time.start));
    let mut imported_count = 0;

    for c in changes {
        // the baseline record stands for the whole tree, its path is "."
        let baseline = c.op == "baseline" && c.path == ".";
        if !baseline && (!is_safe_rel_path(&c.path) || c.old_path.as_deref().is_some_and(|p| !is_safe_rel_path(p))) {
            continue;
        }
        if db.insert_change_imported(&c).map_err(db_err)? {
            imported_count += 1;
            apply_imported_change_to_files(&c, db, blob_store)?;
        }
    }

    Ok(imported_count)
}

/// An unreadable version is content a batch never carried (secret, big, binary): no line map then. A failed write is an error.
fn apply_imported_change_to_files(c: &Change, db: &mut Database, blob_store: &BlobStore) -> io::Result<()> {
    let record = |h: &String| FileRecord { path: c.path.clone(), blob: h.clone(), size: 0, mtime_ns: 0 };
    match c.op.as_str() {
        "create" => {
            if let Some(ref h) = c.after {
                if let Some(s) = blob_store.read(h).ok().and_then(|b| String::from_utf8(b).ok()) {
                    let line_count = s.lines().count();
                    if line_count > 0 {
                        let mut map = crate::linemap::LineMap::new();
                        map.push_run(1, line_count as u32);
                        db.set_file_line_map(&c.path, &map, &[(1, c.id.as_str())]).map_err(db_err)?;
                    }
                }
                db.set_file(&record(h)).map_err(db_err)?;
            }
        }
        "modify" => {
            let text = |h: &Option<String>| h.as_deref().and_then(|h| blob_store.read(h).ok().and_then(|b| String::from_utf8(b).ok()));
            let (before_str, after_str) = (text(&c.before), text(&c.after));
            let old_map = db.get_file_line_map(&c.path).map_err(db_err)?.unwrap_or_default();
            let existing_changes = db.get_file_changes_map(&c.path).map_err(db_err)?;
            let next_seq = existing_changes.keys().copied().max().unwrap_or(0) + 1;
            let new_map = old_map.apply_diff(before_str.as_deref(), after_str.as_deref(), next_seq);
            db.set_file_line_map(&c.path, &new_map, &[(next_seq, c.id.as_str())]).map_err(db_err)?;
            if let Some(ref h) = c.after {
                db.set_file(&record(h)).map_err(db_err)?;
            }
        }
        "delete" => {
            db.delete_file_lines(&c.path).map_err(db_err)?;
            db.delete_file(&c.path).map_err(db_err)?;
        }
        "rename" => {
            if let Some(ref old_path) = c.old_path {
                db.delete_file(old_path).map_err(db_err)?;
                db.rename_file_lines(old_path, &c.path).map_err(db_err)?;
            }
            if let Some(ref h) = c.after {
                db.set_file(&record(h)).map_err(db_err)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn fetch(repo_root: &Path, data_dir: &Path) -> io::Result<usize> {
    let cfg = match read_tracewiseconfig(repo_root) {
        Some(c) => c,
        None => return Ok(0),
    };
    if !may_sync_from(repo_root, data_dir, &cfg.remote) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "remote '{}' comes from .tracewiseconfig and is not approved on this machine; check it, then run `tracewise share trust` and `tracewise fetch`",
                cfg.remote
            ),
        ));
    }

    let remote_dir = data_dir.join("remote");
    ensure_companion_repo(&cfg.remote, &remote_dir)?;
    run_git(
        &remote_dir,
        &[
            "fetch",
            "origin",
            "+refs/heads/tracewise/device/*:refs/heads/tracewise/device/*",
            "+refs/tracewise/device/*:refs/tracewise/device/*",
        ],
    )
    .map_err(|e| io::Error::other(format!("could not fetch from '{}': {e}", cfg.remote)))?;
    copy_remote_batches(&remote_dir, data_dir)?;
    import_all_batches(data_dir, repo_root, true)
}

/// Imports every local batch the database does not hold yet, in one transaction: a full rebuild into an empty database,
/// nothing removed from an existing one. On any failure the database is left as it was.
pub fn rebuild(repo_root: &Path, data_dir: &Path) -> io::Result<usize> {
    let remote_dir = data_dir.join("remote");
    if remote_dir.is_dir() {
        copy_remote_batches(&remote_dir, data_dir)?;
    }
    let has_batches = fs::read_dir(data_dir.join("batches"))
        .map(|mut d| d.any(|e| e.is_ok_and(|e| e.path().join("batch.json").is_file())))
        .unwrap_or(false);
    if !has_batches {
        return Err(io::Error::other("there are no local batches to rebuild from; the history was left untouched"));
    }
    import_all_batches(data_dir, repo_root, false)
}
