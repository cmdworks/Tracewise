//! Local domain sockets and peer identity verification.
use crate::{PlatformError, Result};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

/// Return the effective user ID of the current process.
pub fn current_uid() -> u32 {
    #[cfg(unix)]
    unsafe {
        libc::getuid() as u32
    }
    #[cfg(not(unix))]
    0
}

/// Bind a Unix domain socket at `path` (removing stale socket file first).
#[cfg(unix)]
pub fn listen_unix(path: &Path) -> Result<UnixListener> {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    let listener = UnixListener::bind(path)?;
    Ok(listener)
}

/// Bind a Unix domain socket with specific filesystem permissions (e.g. 0o600).
#[cfg(unix)]
pub fn listen_restricted(path: &Path, mode: u32) -> Result<UnixListener> {
    use std::os::unix::fs::PermissionsExt;
    let listener = listen_unix(path)?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    Ok(listener)
}

/// Connect to a Unix domain socket at `path`.
#[cfg(unix)]
pub fn connect_unix(path: &Path) -> Result<UnixStream> {
    let stream = UnixStream::connect(path)?;
    Ok(stream)
}

/// Retrieve the effective user ID (euid) of the peer connected to `stream`.
#[cfg(unix)]
pub fn peer_uid(stream: &UnixStream) -> Result<u32> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();

    #[cfg(target_os = "macos")]
    {
        let mut euid: libc::uid_t = 0;
        let mut egid: libc::gid_t = 0;
        let ret = unsafe { libc::getpeereid(fd, &mut euid, &mut egid) };
        if ret != 0 {
            return Err(PlatformError::from(std::io::Error::last_os_error()));
        }
        Ok(euid as u32)
    }

    #[cfg(target_os = "linux")]
    {
        let mut ucred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let ret = unsafe { libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_PEERCRED, &mut ucred as *mut _ as *mut libc::c_void, &mut len) };
        if ret != 0 {
            return Err(PlatformError::from(std::io::Error::last_os_error()));
        }
        Ok(ucred.uid as u32)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = fd;
        Err(PlatformError::unsupported("peer_uid"))
    }
}

#[cfg(not(unix))]
pub fn peer_uid_unsupported() -> Result<u32> {
    Err(PlatformError::unsupported("local_socket"))
}

#[cfg(test)]
#[path = "test_socket.rs"]
mod tests;
