//! Scanning specific paths against the database.
use super::apply::{record_create, record_delete, record_modify, record_rename, Stamp};
use super::repo::{detect_renames, CreatedFile};
use super::{ScanOptions, Snapshot};
use crate::blob::BlobStore;
use crate::db::Database;
use crate::diff;
use crate::model::FileRecord;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn scan_specific_paths(
    work_tree: &Path,
    data_dir: &Path,
    branch: &str,
    head: &str,
    options: &ScanOptions,
    paths: &[PathBuf],
) -> io::Result<Vec<String>> {
    let mut db = Database::open_hot(&data_dir.join("trail.db")).map_err(|e| io::Error::other(e.to_string()))?;
    scan_specific_paths_with_db(work_tree, data_dir, branch, head, options, paths, &mut db)
}

struct PathScanContext<'a> {
    pub blobs: &'a BlobStore,
    pub db: &'a mut Database,
    pub options: &'a ScanOptions,
    pub branch: &'a str,
    pub head: &'a str,
    pub device: &'a str,
    pub result_ids: &'a mut Vec<String>,
}

pub fn scan_specific_paths_with_db(
    work_tree: &Path,
    data_dir: &Path,
    branch: &str,
    head: &str,
    options: &ScanOptions,
    paths: &[PathBuf],
    db: &mut Database,
) -> io::Result<Vec<String>> {
    let blobs = BlobStore::new(data_dir);
    let device = crate::get_device_id();
    let mut result_ids = Vec::new();

    db.begin_batch().map_err(|e| io::Error::other(e.to_string()))?;

    let mut ctx = PathScanContext { blobs: &blobs, db, options, branch, head, device: &device, result_ids: &mut result_ids };

    let mut rules = crate::ignore_rules::IgnoreRules::load(work_tree);
    let res = (|| -> io::Result<()> {
        let renamed = record_path_renames(&mut ctx, &mut rules, work_tree, paths)?;
        for p in paths {
            if renamed.contains(&p.to_string_lossy().to_string()) {
                continue;
            }
            let path_str = p.to_string_lossy().to_string();
            if !crate::share::is_safe_rel_path(&path_str) || tracewise_platform::paths::is_os_junk(p) {
                continue;
            }
            let full_path = work_tree.join(p);
            let known = ctx.db.get_file(&path_str).map_err(|e| io::Error::other(e.to_string()))?;
            // ignored files are never recorded (a watcher event for target/ or node_modules/ is noise), unless they were recorded before
            if known.is_none() && rules.is_ignored(p) {
                continue;
            }

            match fs::symlink_metadata(&full_path) {
                Ok(m) if m.is_file() || m.file_type().is_symlink() => scan_single_file(&mut ctx, &full_path, &path_str, known.as_ref())?,
                Ok(_) => {}
                Err(e) if is_absent(&e) => {
                    if let Some(known) = known {
                        scan_deleted_file(&mut ctx, &path_str, &known)?;
                    }
                }
                // permission denied or an I/O error does not prove the file is gone: stop and record nothing for this scan
                Err(e) => return Err(e),
            }
        }
        Ok(())
    })();

    if let Err(e) = res {
        let _ = ctx.db.rollback_batch();
        return Err(e);
    }
    ctx.db.commit_batch().map_err(|e| io::Error::other(e.to_string()))?;
    Ok(result_ids)
}

/// Only "no such file" (or a path through something that is no longer a folder) proves a file is gone.
pub(super) fn is_absent(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotFound || e.raw_os_error() == Some(20)
}

/// A file that vanished and another with the same content that appeared in the same call is a rename (`git mv`, `mv`).
/// Returns the paths (old and new) that are handled.
fn record_path_renames(
    ctx: &mut PathScanContext,
    rules: &mut crate::ignore_rules::IgnoreRules,
    work_tree: &Path,
    paths: &[PathBuf],
) -> io::Result<HashSet<String>> {
    let mut gone = Vec::new();
    let mut fresh = Vec::new();
    for p in paths {
        let path_str = p.to_string_lossy().to_string();
        if !crate::share::is_safe_rel_path(&path_str) || tracewise_platform::paths::is_os_junk(p) {
            continue;
        }
        let known = ctx.db.get_file(&path_str).map_err(|e| io::Error::other(e.to_string()))?;
        // same filter as the main loop: an ignored new file must not be paired with a vanished one
        if known.is_none() && rules.is_ignored(p) {
            continue;
        }
        let full = work_tree.join(p);
        match (known, fs::symlink_metadata(&full)) {
            (Some(k), Err(e)) if is_absent(&e) => gone.push((path_str, k.blob)),
            (Some(_), Err(e)) => return Err(e),
            (None, Ok(_)) => {
                if let Some(snap) = Snapshot::take(&full, ctx.blobs.max_file(), |_, _, _| false)? {
                    fresh.push(CreatedFile {
                        path: path_str,
                        hash: snap.hash,
                        bytes: Vec::new(),
                        mtime_ns: snap.mtime_ns,
                        size: snap.size,
                    });
                }
            }
            _ => {}
        }
    }
    let mut handled = HashSet::new();
    if gone.is_empty() || fresh.is_empty() {
        return Ok(handled);
    }
    let (renames, _, _) = detect_renames(&fresh, &gone, ctx.db, ctx.options, ctx.branch, ctx.head, ctx.device);
    for r in renames {
        record_rename(ctx.db, ctx.result_ids, &r.change, &r.del_path)?;
        ctx.db.set_file(&r.file_record).map_err(|e| io::Error::other(e.to_string()))?;
        let mode = Snapshot::take(&work_tree.join(&r.file_record.path), ctx.blobs.max_file(), |_, _, _| true)?.map(|s| s.mode).unwrap_or(0);
        ctx.db.set_file_mode(&r.file_record.path, mode).map_err(|e| io::Error::other(e.to_string()))?;
        handled.insert(r.del_path);
        handled.insert(r.file_record.path);
    }
    Ok(handled)
}

fn scan_single_file(ctx: &mut PathScanContext, full_path: &Path, path_str: &str, known: Option<&FileRecord>) -> io::Result<()> {
    let Some(snap) = Snapshot::take(full_path, ctx.blobs.max_file(), |_, _, _| false)? else { return Ok(()) };
    let Snapshot { bytes, hash: current_hash, size, mtime_ns, mode } = snap;
    let known_mode = ctx.db.get_file_mode(path_str).map_err(|e| io::Error::other(e.to_string()))?;
    ctx.db.set_file_mode(path_str, mode).map_err(|e| io::Error::other(e.to_string()))?;

    let stamp = Stamp { options: ctx.options, branch: ctx.branch, head: ctx.head, device: ctx.device };
    if let Some(known) = known {
        if current_hash == known.blob && known_mode.is_some_and(|m| m != mode) {
            let change = stamp.change(ctx.db, path_str, "chmod", true, (Vec::new(), Some(known.blob.clone()), Some(current_hash.clone())));
            ctx.result_ids.push(change.id.clone());
            ctx.db.insert_change(&change).map_err(|e| io::Error::other(e.to_string()))?;
            ctx.db
                .set_file(&FileRecord { path: path_str.to_string(), blob: current_hash, size, mtime_ns })
                .map_err(|e| io::Error::other(e.to_string()))?;
        } else if current_hash != known.blob {
            let before_bytes = ctx.blobs.read(&known.blob).ok();
            let hunks = diff::compute_hunks(before_bytes.as_deref(), Some(&bytes));
            ctx.blobs.store(&bytes)?;

            let change = stamp.change(ctx.db, path_str, "modify", true, (hunks, Some(known.blob.clone()), Some(current_hash.clone())));

            record_modify(ctx.db, ctx.result_ids, &change, before_bytes.as_deref(), &bytes)?;
            ctx.db
                .set_file(&FileRecord { path: path_str.to_string(), blob: current_hash, size, mtime_ns })
                .map_err(|e| io::Error::other(e.to_string()))?;
        }
    } else {
        let hunks = diff::compute_hunks(None, Some(&bytes));
        ctx.blobs.store(&bytes)?;

        let change = stamp.change(ctx.db, path_str, "create", false, (hunks, None, Some(current_hash.clone())));

        record_create(ctx.db, ctx.result_ids, &change, &bytes)?;
        ctx.db
            .set_file(&FileRecord { path: path_str.to_string(), blob: current_hash, size, mtime_ns })
            .map_err(|e| io::Error::other(e.to_string()))?;
    }
    Ok(())
}

fn scan_deleted_file(ctx: &mut PathScanContext, path_str: &str, known: &FileRecord) -> io::Result<()> {
    let stamp = Stamp { options: ctx.options, branch: ctx.branch, head: ctx.head, device: ctx.device };
    let before_bytes = ctx.blobs.read(&known.blob).ok();
    let hunks = diff::compute_hunks(before_bytes.as_deref(), None);

    let change = stamp.change(ctx.db, path_str, "delete", false, (hunks, Some(known.blob.clone()), None));

    record_delete(ctx.db, ctx.result_ids, &change)
}
