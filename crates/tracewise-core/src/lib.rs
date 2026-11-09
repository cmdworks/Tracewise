//! Core domain logic, storage, and models for Tracewise.

pub mod attribution;
pub mod blob;
pub mod clock;
pub mod db;
pub mod diff;
pub mod identity;
pub mod ignore_rules;
pub mod linemap;
pub mod lines;
pub mod model;
pub mod predict;
pub mod profile;
pub mod reasons;
pub mod repos;
pub mod safe_path;
pub mod scan;
pub mod share;
#[cfg(test)]
mod test_env;

pub use attribution::attribute_window;

pub use db::{Database, VerifyReport};
pub use linemap::{LineMap, LineMapRun};
pub use model::{glob_match, validate_why, CommandRecord, Intent, LockRecord, PendingItem, Window};
pub use predict::predict_written_files;
pub use reasons::{get_reasons_mode, get_reasons_mode_for, ReasonsMode};
pub use scan::{scan_repository, scan_repository_with_db, scan_specific_paths, scan_specific_paths_with_db, ScanOptions};

use blob::BlobStore;
use model::{Actor, Change, FileRecord, GitInfo, TimeRange};
use std::io;
use std::path::{Path, PathBuf};

/// Returns the global TRACEWISE_HOME path (defaults to ~/.tracewise).
pub fn get_tracewise_home() -> PathBuf {
    tracewise_platform::paths::tracewise_home_dir()
}

/// Gets the device ID from `TRACEWISE_DEVICE` env or `TRACEWISE_HOME/device`.
pub fn get_device_id() -> String {
    if let Ok(dev) = std::env::var("TRACEWISE_DEVICE") {
        if !dev.trim().is_empty() {
            return dev.trim().to_string();
        }
    }

    let tracewise_home = get_tracewise_home();

    let dev_file = tracewise_home.join("device");
    if let Ok(content) = std::fs::read_to_string(&dev_file) {
        if !content.trim().is_empty() {
            return content.trim().to_string();
        }
    }

    let dev_id = format!("dev-{}", &ulid::Ulid::new().to_string().to_lowercase()[..8]);
    let _ = tracewise_platform::private::create_dir_all(&tracewise_home);
    let _ = tracewise_platform::private::write(&dev_file, dev_id.as_bytes());
    dev_id
}

/// Records baseline changes for all unrecorded non-ignored files in `work_tree`.
pub fn record_baseline(work_tree: &Path, data_dir: &Path, branch: &str, head: &str, files: &[PathBuf]) -> io::Result<usize> {
    let db = Database::open(&data_dir.join("tracewise.db")).map_err(|e| io::Error::other(e.to_string()))?;
    let blobs = BlobStore::new(data_dir);
    let device = get_device_id();
    let known = db.get_known_files().map_err(|e| io::Error::other(e.to_string()))?;

    let mut count = 0;
    let mut generator = ulid::Generator::new();

    db.begin_batch().map_err(|e| io::Error::other(e.to_string()))?;

    for file in files {
        let path_str = file.to_string_lossy().to_string();
        if known.contains_key(&path_str) {
            continue;
        }

        let full_path = work_tree.join(file);
        let Some(snap) = scan::Snapshot::take(&full_path, blobs.max_file(), |_, _, _| false)? else { continue };
        if snap.size <= blobs.max_file() && snap.mode != 2 {
            blobs.store(&snap.bytes)?;
        }
        db.set_file(&FileRecord { path: path_str.clone(), blob: snap.hash, size: snap.size, mtime_ns: snap.mtime_ns })
            .map_err(|e| io::Error::other(e.to_string()))?;
        db.set_file_mode(&path_str, snap.mode).map_err(|e| io::Error::other(e.to_string()))?;

        // NOTE: Baseline files do not write to line_map or per-file changes (L01, L02).
        // Untouched files cost 0 rows in line_map and are resolved as baseline on blame/why.
        count += 1;
    }

    if count > 0 {
        let summary_change = Change {
            id: generator.generate().map(|u| u.to_string()).unwrap_or_else(|_| ulid::Ulid::new().to_string()),
            time: TimeRange::now(),
            path: ".".to_string(),
            op: "baseline".to_string(),
            old_path: None,
            hunks: Vec::new(),
            before: None,
            after: None,
            actor: Actor::baseline(),
            candidates: Vec::new(),
            confidence: "exact".to_string(),
            source: "baseline".to_string(),
            command: None,
            intent: None,
            lock_conflict: None,
            device: device.clone(),
            git: GitInfo { branch: branch.to_string(), head: head.to_string() },
            prev_hash: None,
            undoes: None,
        };

        db.insert_change(&summary_change).map_err(|e| io::Error::other(e.to_string()))?;
    }

    db.commit_batch().map_err(|e| io::Error::other(e.to_string()))?;

    Ok(count)
}

#[cfg(test)]
#[path = "test_lib.rs"]
mod tests;
