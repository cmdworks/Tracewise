//! Repository-wide file scanning and change detection.
use super::actor::{check_lock_conflict, resolve_actor_candidates_confidence};
use super::apply::{record_create, record_delete, record_modify, record_rename, Stamp};
use super::{ScanOptions, Snapshot};
use crate::blob::BlobStore;
use crate::db::Database;
use crate::diff;
use crate::model::{Change, FileRecord, GitInfo, TimeRange};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

pub(crate) struct CreatedFile {
    pub path: String,
    pub hash: String,
    pub bytes: Vec<u8>,
    pub mtime_ns: i64,
    pub size: u64,
}

pub(crate) struct ModifiedFile {
    pub change: Change,
    /// Only the mode changed (`chmod`): the lines are left alone.
    pub meta_only: bool,
    pub before_bytes: Option<Vec<u8>>,
    pub after_bytes: Vec<u8>,
}

pub(crate) struct RenameMatch {
    pub change: Change,
    pub del_path: String,
    pub file_record: FileRecord,
}

pub fn scan_repository(
    work_tree: &Path,
    data_dir: &Path,
    branch: &str,
    head: &str,
    options: &ScanOptions,
    discovered_files: &[PathBuf],
) -> io::Result<Vec<String>> {
    let mut db = Database::open(&data_dir.join("tracewise.db")).map_err(|e| io::Error::other(e.to_string()))?;
    scan_repository_with_db(work_tree, data_dir, branch, head, options, discovered_files, &mut db)
}

struct RepoScanContext<'a> {
    pub work_tree: &'a Path,
    pub blobs: &'a BlobStore,
    pub db: &'a Database,
    pub options: &'a ScanOptions,
    pub branch: &'a str,
    pub head: &'a str,
    pub device: &'a str,
}

struct RepoChanges {
    pub renames: Vec<RenameMatch>,
    pub modified: Vec<ModifiedFile>,
    pub created: Vec<CreatedFile>,
    pub matched_created: HashSet<usize>,
    pub deleted: Vec<(String, String)>,
    pub matched_deleted: HashSet<usize>,
    pub files_to_update: Vec<FileRecord>,
    pub modes: HashMap<String, u8>,
}

pub fn scan_repository_with_db(
    work_tree: &Path,
    data_dir: &Path,
    branch: &str,
    head: &str,
    options: &ScanOptions,
    discovered_files: &[PathBuf],
    db: &mut Database,
) -> io::Result<Vec<String>> {
    let blobs = BlobStore::new(data_dir);
    let device = crate::get_device_id();
    db.begin_batch().map_err(|e| io::Error::other(e.to_string()))?;
    let scanned = scan_with_lock(work_tree, &blobs, branch, head, options, discovered_files, db, &device);
    match scanned {
        Ok(ids) => db.commit_batch().map(|_| ids).map_err(|e| io::Error::other(e.to_string())),
        Err(e) => {
            let _ = db.rollback_batch();
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_with_lock(
    work_tree: &Path,
    blobs: &BlobStore,
    branch: &str,
    head: &str,
    options: &ScanOptions,
    discovered_files: &[PathBuf],
    db: &mut Database,
    device: &str,
) -> io::Result<Vec<String>> {
    let known_files = db.get_known_files().map_err(|e| io::Error::other(e.to_string()))?;
    let known_modes = db.get_file_modes().map_err(|e| io::Error::other(e.to_string()))?;

    let ctx = RepoScanContext { work_tree, blobs, db, options, branch, head, device };

    let (created, modified, mut files_to_update, modes) = scan_disk_files(&ctx, &known_files, &known_modes, discovered_files)?;

    let current_files_set: HashSet<String> = discovered_files.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let deleted: Vec<(String, String)> = known_files
        .into_iter()
        .filter(|(p, _)| !current_files_set.contains(p) && !cannot_inspect(work_tree, p))
        .map(|(p, k)| (p, k.blob))
        .collect();

    let (renames, matched_created, matched_deleted) = detect_renames(&created, &deleted, db, options, branch, head, device);

    for r in &renames {
        files_to_update.push(r.file_record.clone());
    }

    let changes = RepoChanges { renames, modified, created, matched_created, deleted, matched_deleted, files_to_update, modes };

    commit_repo_changes(db, blobs, options, branch, head, device, changes)
}

/// True when the path exists but cannot be looked at (an unreadable folder): not proof that the file is gone.
fn cannot_inspect(work_tree: &Path, rel: &str) -> bool {
    std::fs::symlink_metadata(work_tree.join(rel)).is_err_and(|e| !super::paths::is_absent(&e))
}

/// New files, changed files, file records to refresh, and the kind (mode) of each path that was read.
type DiskScan = (Vec<CreatedFile>, Vec<ModifiedFile>, Vec<FileRecord>, HashMap<String, u8>);

fn scan_disk_files(
    ctx: &RepoScanContext,
    known_files: &HashMap<String, FileRecord>,
    known_modes: &HashMap<String, u8>,
    discovered_files: &[PathBuf],
) -> io::Result<DiskScan> {
    let mut created = Vec::new();
    let mut modified = Vec::new();
    let mut files_to_update = Vec::new();
    let mut modes = HashMap::new();

    for file in discovered_files {
        let path_str = file.to_string_lossy().to_string();
        let known = known_files.get(&path_str);
        let known_mode = known_modes.get(&path_str).copied();
        let same = |size: u64, mtime_ns: i64, mode: u8| {
            known.is_some_and(|k| k.mtime_ns == mtime_ns && k.size == size) && known_mode == Some(mode)
        };
        let Some(snap) = Snapshot::take(&ctx.work_tree.join(file), ctx.blobs.max_file(), same)? else { continue };
        if snap.unchanged() {
            continue;
        }
        let Snapshot { bytes, hash: current_hash, size, mtime_ns, mode } = snap;
        modes.insert(path_str.clone(), mode);

        if let Some(known) = known {
            let mode_only = current_hash == known.blob && known_mode.is_some_and(|m| m != mode);
            if current_hash != known.blob || mode_only {
                let before_bytes = if mode_only { None } else { ctx.blobs.read(&known.blob).ok() };
                let hunks = if mode_only { Vec::new() } else { diff::compute_hunks(before_bytes.as_deref(), Some(&bytes)) };
                ctx.blobs.store(&bytes)?;

                let default_op = if mode_only { "chmod" } else { "modify" };
                let stamp = Stamp { options: ctx.options, branch: ctx.branch, head: ctx.head, device: ctx.device };
                let change =
                    stamp.change(ctx.db, &path_str, default_op, true, (hunks, Some(known.blob.clone()), Some(current_hash.clone())));

                modified.push(ModifiedFile { change, before_bytes, after_bytes: bytes, meta_only: mode_only });
                files_to_update.push(FileRecord { path: path_str, blob: current_hash, size, mtime_ns });
            } else {
                files_to_update.push(FileRecord { path: path_str, blob: current_hash, size, mtime_ns });
            }
        } else {
            created.push(CreatedFile { path: path_str, hash: current_hash, bytes, mtime_ns, size });
        }
    }

    Ok((created, modified, files_to_update, modes))
}

pub(crate) fn detect_renames(
    created: &[CreatedFile],
    deleted: &[(String, String)],
    db: &Database,
    options: &ScanOptions,
    branch: &str,
    head: &str,
    device: &str,
) -> (Vec<RenameMatch>, HashSet<usize>, HashSet<usize>) {
    let mut renames = Vec::new();
    let mut matched_created = HashSet::new();
    let mut matched_deleted = HashSet::new();

    let mut created_by_hash: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, c) in created.iter().enumerate() {
        created_by_hash.entry(c.hash.clone()).or_default().push(i);
    }

    for (del_idx, (del_path, del_hash)) in deleted.iter().enumerate() {
        if let Some(c_indices) = created_by_hash.get_mut(del_hash) {
            if let Some(c_idx) = c_indices.pop() {
                matched_deleted.insert(del_idx);
                matched_created.insert(c_idx);

                let c = &created[c_idx];
                let (actor, candidates, confidence, source_hint) = resolve_actor_candidates_confidence(db, options, &c.path, false);
                let src = source_hint.unwrap_or_else(|| options.source.clone());

                let change = Change {
                    id: ulid::Ulid::new().to_string(),
                    time: TimeRange::now(),
                    path: c.path.clone(),
                    op: options.op.clone().unwrap_or_else(|| "rename".to_string()),
                    old_path: Some(del_path.clone()),
                    hunks: Vec::new(),
                    before: Some(c.hash.clone()),
                    after: Some(c.hash.clone()),
                    actor: actor.clone(),
                    candidates,
                    confidence,
                    source: src,
                    command: None,
                    intent: None,
                    lock_conflict: check_lock_conflict(db, &c.path, &actor),
                    device: device.to_string(),
                    git: GitInfo { branch: branch.to_string(), head: head.to_string() },
                    prev_hash: None,
                    undoes: options.undoes.clone(),
                };

                renames.push(RenameMatch {
                    change,
                    del_path: del_path.clone(),
                    file_record: FileRecord { path: c.path.clone(), blob: c.hash.clone(), size: c.size, mtime_ns: c.mtime_ns },
                });
            }
        }
    }

    (renames, matched_created, matched_deleted)
}

fn commit_repo_changes(
    db: &mut Database,
    blobs: &BlobStore,
    options: &ScanOptions,
    branch: &str,
    head: &str,
    device: &str,
    changes: RepoChanges,
) -> io::Result<Vec<String>> {
    let mut result_ids = Vec::new();
    let mut files_to_update = changes.files_to_update;
    let stamp = Stamp { options, branch, head, device };

    let write_res = (|| -> io::Result<()> {
        for r in &changes.renames {
            record_rename(db, &mut result_ids, &r.change, &r.del_path)?;
        }
        for m in &changes.modified {
            if m.meta_only {
                db.insert_change(&m.change).map_err(|e| io::Error::other(e.to_string()))?;
                result_ids.push(m.change.id.clone());
            } else {
                record_modify(db, &mut result_ids, &m.change, m.before_bytes.as_deref(), &m.after_bytes)?;
            }
        }
        for (i, c) in changes.created.into_iter().enumerate() {
            if changes.matched_created.contains(&i) {
                continue;
            }
            blobs.store(&c.bytes)?;
            let hunks = diff::compute_hunks(None, Some(&c.bytes));
            let change = stamp.change(db, &c.path, "create", false, (hunks, None, Some(c.hash.clone())));
            record_create(db, &mut result_ids, &change, &c.bytes)?;
            files_to_update.push(FileRecord { path: c.path, blob: c.hash, size: c.size, mtime_ns: c.mtime_ns });
        }
        for (del_idx, (del_path, del_hash)) in changes.deleted.into_iter().enumerate() {
            if changes.matched_deleted.contains(&del_idx) {
                continue;
            }
            let before_bytes = blobs.read(&del_hash).ok();
            let hunks = diff::compute_hunks(before_bytes.as_deref(), None);
            let change = stamp.change(db, &del_path, "delete", false, (hunks, Some(del_hash), None));
            record_delete(db, &mut result_ids, &change)?;
        }
        for f in &files_to_update {
            db.set_file(f).map_err(|e| io::Error::other(e.to_string()))?;
            if let Some(mode) = changes.modes.get(&f.path) {
                db.set_file_mode(&f.path, *mode).map_err(|e| io::Error::other(e.to_string()))?;
            }
        }
        Ok(())
    })();

    write_res?;
    Ok(result_ids)
}
