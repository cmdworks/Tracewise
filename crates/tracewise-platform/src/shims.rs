//! Shim linking: creating symlinks on Unix or falling back to copies.
use crate::{PlatformError, Result};
use std::fs;
use std::path::Path;

/// Link or copy a shim entry binary to `link_path`.
pub fn create_shim_link(target: &Path, link_path: &Path) -> Result<()> {
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // If destination already exists, remove it first
    if link_path.is_symlink() || link_path.exists() {
        let _ = fs::remove_file(link_path);
    }

    #[cfg(unix)]
    {
        if let Err(sym_err) = std::os::unix::fs::symlink(target, link_path) {
            // Attempt copy fallback on symlink failure
            if fs::copy(target, link_path).is_err() {
                return Err(PlatformError::from(sym_err));
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    {
        // On Windows without Developer Mode, symlinks require admin; copy fallback
        fs::copy(target, link_path)?;
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    {
        Err(PlatformError::unsupported("shim_linking"))
    }
}

/// Set permissions mode on Unix; no-op on Windows/other.
pub fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(mode);
        fs::set_permissions(path, perms)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

/// Set executable permissions (0o755) on Unix; no-op on Windows/other.
pub fn set_executable(path: &Path) -> Result<()> {
    set_mode(path, 0o755)
}

/// Explicit copy fallback for platforms/environments without symlink privileges.
pub fn create_shim_copy(target: &Path, dest_path: &Path) -> Result<()> {
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if dest_path.exists() {
        let _ = fs::remove_file(dest_path);
    }
    fs::copy(target, dest_path)?;
    Ok(())
}

#[cfg(test)]
#[path = "test_shims.rs"]
mod tests;
