//! Working tree scanning and change detection.
use crate::model::Actor;
use std::collections::HashSet;

pub(crate) mod actor;
pub(crate) mod apply;
pub(crate) mod paths;
pub(crate) mod repo;
#[cfg(test)]
mod tests;

pub use paths::{scan_specific_paths, scan_specific_paths_with_db};
pub use repo::{scan_repository, scan_repository_with_db};

/// What one path looks like now: content (a link's target for a symbolic link), hash, size, mtime and kind.
pub(crate) struct Snapshot {
    pub bytes: Vec<u8>,
    pub hash: String,
    pub size: u64,
    pub mtime_ns: i64,
    pub mode: u8,
}

impl Snapshot {
    /// `Ok(None)` when the path is gone or is not a file or link. A file over `max_file` is only hashed (Y17).
    pub(crate) fn take(
        full_path: &std::path::Path,
        max_file: u64,
        known_unchanged: impl Fn(u64, i64, u8) -> bool,
    ) -> std::io::Result<Option<Snapshot>> {
        let Ok(meta) = std::fs::symlink_metadata(full_path) else { return Ok(None) };
        let mode = tracewise_platform::private::file_kind(&meta);
        let mtime_ns =
            meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos() as i64).unwrap_or(0);
        let size = meta.len();
        if mode != 2 && !meta.is_file() {
            return Ok(None);
        }
        if mtime_ns != 0 && known_unchanged(size, mtime_ns, mode) {
            return Ok(Some(Snapshot { bytes: Vec::new(), hash: String::new(), size, mtime_ns, mode }));
        }
        let (bytes, hash) = if mode == 2 {
            let target = std::fs::read_link(full_path)?.to_string_lossy().into_owned().into_bytes();
            // hashed, never stored: undo and restore then refuse a link as "recorded by hash only" instead of writing a file
            (Vec::new(), crate::blob::hash_bytes(&target))
        } else {
            crate::blob::read_or_hash(full_path, size, max_file)?
        };
        Ok(Some(Snapshot { bytes, hash, size, mtime_ns, mode }))
    }

    /// `take` found nothing new (size, mtime and kind equal the stored ones): `hash` is empty.
    pub(crate) fn unchanged(&self) -> bool {
        self.hash.is_empty()
    }
}

/// Options for scanning a repository.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub source: String,
    pub actor: Option<Actor>,
    pub confidence: Option<String>,
    pub predicted_paths: Option<HashSet<String>>,
    pub window_start: Option<String>,
    pub op: Option<String>,
    pub undoes: Option<Vec<String>>,
    pub path_undoes: Option<std::collections::HashMap<String, Vec<String>>>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            source: "scan".to_string(),
            actor: None,
            confidence: None,
            predicted_paths: None,
            window_start: None,
            op: None,
            undoes: None,
            path_undoes: None,
        }
    }
}
