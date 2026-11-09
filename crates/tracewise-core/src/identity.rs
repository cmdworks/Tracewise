//! Who a Tracewise data folder belongs to: an id and the work-tree path it was made for.
//!
//! Copying a repository with its `.git` folder copies the Tracewise history too, and two copies would then record
//! into separate histories under the same id and push to the same device branch. The stamp tells a **copy**
//! (the original is still there with the same id) from a **move** (the old path is gone).
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyState {
    /// This is the folder the data was made for.
    Original,
    /// The folder was moved (the old path no longer holds this id); the stamp has been updated.
    Moved,
    /// A copy: `original` still exists with the same id.
    Copy { original: PathBuf },
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn read(data_dir: &Path, name: &str) -> Option<String> {
    fs::read_to_string(data_dir.join(name)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The repository id, created (with the path stamp) on first use.
pub fn repo_id(data_dir: &Path, main_work_tree: &Path) -> String {
    if let Some(id) = read(data_dir, "id") {
        if read(data_dir, "home").is_none() {
            let _ = tracewise_platform::private::write(data_dir.join("home"), canonical(main_work_tree).to_string_lossy().as_bytes());
        }
        return id;
    }
    let id = ulid::Ulid::new().to_string();
    let _ = tracewise_platform::private::create_dir_all(data_dir);
    let _ = tracewise_platform::private::write(data_dir.join("id"), &id);
    let _ = tracewise_platform::private::write(data_dir.join("home"), canonical(main_work_tree).to_string_lossy().as_bytes());
    id
}

/// Compares the stamp with where this data folder is now. `main_work_tree` is the repository's own work tree
/// (not a linked worktree: those share the data folder and are not copies).
pub fn check_copy(data_dir: &Path, main_work_tree: &Path) -> CopyState {
    let id = repo_id(data_dir, main_work_tree);
    let here = canonical(main_work_tree);
    let Some(home) = read(data_dir, "home").map(PathBuf::from) else {
        return CopyState::Original;
    };
    if home == here {
        return CopyState::Original;
    }
    let same_id_there = fs::read_to_string(home.join(".git").join("trail").join("id")).map(|s| s.trim() == id).unwrap_or(false);
    if same_id_there {
        CopyState::Copy { original: home }
    } else {
        let _ = tracewise_platform::private::write(data_dir.join("home"), here.to_string_lossy().as_bytes());
        CopyState::Moved
    }
}

/// Gives a copy its own id and its own device name for pushes. Returns (previous id, new id, device).
pub fn adopt_copy(data_dir: &Path, main_work_tree: &Path) -> io::Result<(String, String, String)> {
    let previous = repo_id(data_dir, main_work_tree);
    let new_id = ulid::Ulid::new().to_string();
    let device = format!("{}-copy-{}", crate::get_device_id(), new_id[new_id.len() - 6..].to_lowercase());
    tracewise_platform::private::write(data_dir.join("id"), &new_id)?;
    tracewise_platform::private::write(data_dir.join("home"), canonical(main_work_tree).to_string_lossy().as_bytes())?;
    tracewise_platform::private::write(data_dir.join("device"), &device)?;
    Ok((previous, new_id, device))
}

/// The device name this repository pushes under: its own override (set by `adopt_copy`) or the machine's.
pub fn device_for(data_dir: &Path) -> String {
    read(data_dir, "device").unwrap_or_else(crate::get_device_id)
}

#[cfg(test)]
#[path = "test_identity.rs"]
mod tests;
