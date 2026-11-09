//! Turns a path typed by a person or read from a record into a file inside the work tree, or says why not.
use std::path::{Path, PathBuf};

/// `rel` must be a plain relative name (no `..`, no absolute path, no `.git`), and neither the file nor any folder
/// above it may be a symlink: writing through one could leave the repository. Returns the relative path in a
/// normal form (`./a//b` becomes `a/b`) and the full path.
pub fn resolve_inside(work_tree: &Path, rel: &str) -> Result<(String, PathBuf), String> {
    if rel.starts_with(['/', '\\']) || Path::new(rel).is_absolute() {
        return Err(format!("'{rel}' is not a path inside the repository"));
    }
    let cleaned: Vec<&str> = rel.split('/').filter(|c| !c.is_empty() && *c != ".").collect();
    let normal = cleaned.join("/");
    if !crate::share::is_safe_rel_path(&normal) {
        return Err(format!("'{rel}' is not a path inside the repository"));
    }
    let mut full = work_tree.to_path_buf();
    for part in &cleaned {
        full.push(part);
        if std::fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!("'{rel}' goes through a symlink; refusing to write through it"));
        }
    }
    Ok((normal, full))
}

#[cfg(test)]
#[path = "test_safe_path.rs"]
mod tests;
