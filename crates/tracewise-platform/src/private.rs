//! Files and folders only the owner can use (V02): Tracewise's history can contain source code and secrets.
use std::io;
use std::path::Path;

#[cfg(unix)]
mod imp {
    use super::*;
    use std::fs::{self, DirBuilder, OpenOptions, Permissions};
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

    pub fn write(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> io::Result<()> {
        OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path.as_ref())?.write_all(data.as_ref())?;
        strip(path.as_ref())
    }

    pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        DirBuilder::new().recursive(true).mode(0o700).create(path.as_ref())
    }

    pub fn set_mode(path: impl AsRef<Path>, mode: u32) -> io::Result<()> {
        fs::set_permissions(path, Permissions::from_mode(mode))
    }

    pub fn symlink(target: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    /// Hands a file's (or folder's) data to the drive with `fsync`. Unlike `File::sync_all` (which is `F_FULLFSYNC` on macOS) it
    /// does not flush the drive's own cache; one `sync_all` afterwards covers everything fsynced before it.
    pub fn fsync(path: &Path) -> io::Result<()> {
        use std::os::unix::io::AsRawFd;
        let file = fs::File::open(path)?;
        if unsafe { libc::fsync(file.as_raw_fd()) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Clears the group and other permission bits (the owner's, including `x`, stay).
    pub fn strip(path: &Path) -> io::Result<()> {
        let mode = fs::symlink_metadata(path)?;
        if mode.file_type().is_symlink() {
            return Ok(());
        }
        let bits = mode.permissions().mode();
        if bits & 0o077 != 0 {
            fs::set_permissions(path, Permissions::from_mode(bits & !0o077))?;
        }
        Ok(())
    }
}

#[cfg(not(unix))]
mod imp {
    use super::*;
    pub fn write(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> io::Result<()> {
        std::fs::write(path, data)
    }
    pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
    pub fn fsync(path: &Path) -> io::Result<()> {
        std::fs::File::open(path)?.sync_all()
    }
    pub fn strip(_path: &Path) -> io::Result<()> {
        Ok(())
    }
    pub fn set_mode(_path: impl AsRef<Path>, _mode: u32) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn symlink(_target: impl AsRef<Path>, _link: impl AsRef<Path>) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

/// `set_mode` and `symlink` are the unix-only operations the tests of other crates need, kept here with the rest of the OS code.
pub use imp::{create_dir_all, fsync, set_mode, strip, symlink, write};

/// `strip` for a folder and everything under it (used by `trail gc`, which also covers git's own pack files).
pub fn strip_tree(root: &Path) {
    let _ = strip(root);
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(t) if t.is_dir() => strip_tree(&path),
            Ok(t) if t.is_file() => {
                let _ = strip(&path);
            }
            _ => {}
        }
    }
}

/// 2 for a symbolic link, 1 for a file with an executable bit, else 0 (what Tracewise tracks besides content).
pub fn file_kind(meta: &std::fs::Metadata) -> u8 {
    if meta.file_type().is_symlink() {
        return 2;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        u8::from(meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    0
}
