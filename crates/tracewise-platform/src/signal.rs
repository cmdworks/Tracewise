//! Process signal management: stop, wake, terminate, and status verification.
use crate::{PlatformError, Result};

pub const SIG_TERM: i32 = libc::SIGTERM;
pub const SIG_INT: i32 = libc::SIGINT;
#[cfg(unix)]
pub const SIG_HUP: i32 = libc::SIGHUP;
#[cfg(unix)]
pub const SIG_KILL: i32 = libc::SIGKILL;
// Windows' C runtime has no SIGHUP or SIGKILL; the POSIX numbers keep the stub build compiling.
#[cfg(not(unix))]
pub const SIG_HUP: i32 = 1;
#[cfg(not(unix))]
pub const SIG_KILL: i32 = 9;

/// Check if a process with `pid` is currently alive.
pub fn is_process_running(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(pid, 0) };
        ret == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(windows)]
    {
        false
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

/// Check if a live process PID corresponds to a Tracewise daemon (`tracewise` or `tracewise-watchd`).
pub fn is_daemon_pid(pid: i32) -> bool {
    if !is_process_running(pid) {
        return false;
    }
    if let Some(name) = crate::process::get_proc_name(pid as u32) {
        if name.contains("tracewise") || name.contains("watchd") {
            return true;
        }
    }
    if let Some(path) = crate::process::get_proc_path(pid as u32) {
        if path.contains("tracewise") || path.contains("watchd") {
            return true;
        }
    }
    false
}

/// Send a signal to a process on Unix.
pub fn signal_process(pid: i32, sig: i32) -> Result<()> {
    if pid <= 0 {
        return Err(PlatformError::NotFound(format!("invalid pid {pid}")));
    }
    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(pid, sig) };
        if ret == 0 {
            Ok(())
        } else {
            Err(PlatformError::from(std::io::Error::last_os_error()))
        }
    }
    #[cfg(not(unix))]
    {
        let _ = sig;
        Err(PlatformError::unsupported("signals"))
    }
}

/// Request graceful termination of process (SIGTERM on Unix).
pub fn terminate_process(pid: i32) -> Result<()> {
    signal_process(pid, SIG_TERM)
}

/// Forcibly kill process (SIGKILL on Unix).
pub fn kill_process(pid: i32) -> Result<()> {
    signal_process(pid, SIG_KILL)
}

/// Wake daemon process (SIGHUP on Unix).
/// Criterion P08: Never signal a stale PID pointing to an unrelated process.
pub fn wake_daemon(pid: i32) -> Result<bool> {
    if !is_daemon_pid(pid) {
        return Ok(false);
    }
    signal_process(pid, SIG_HUP).map(|_| true)
}

/// Terminate daemon process (SIGTERM on Unix).
/// Criterion P08: Never signal a stale PID pointing to an unrelated process.
pub fn terminate_daemon(pid: i32) -> Result<bool> {
    if !is_daemon_pid(pid) {
        return Ok(false);
    }
    signal_process(pid, SIG_TERM).map(|_| true)
}

/// Reset SIGPIPE to default action.
pub fn reset_sigpipe() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// Install a signal handler on Unix.
pub fn install_signal_handler(sig: i32, handler: extern "C" fn(i32)) {
    #[cfg(unix)]
    unsafe {
        libc::signal(sig, handler as *const () as libc::sighandler_t);
    }
    #[cfg(not(unix))]
    {
        let _ = (sig, handler);
    }
}

/// Setup SIGINT and SIGTERM handlers for graceful shutdown.
pub fn setup_stop_signals(handler: extern "C" fn(i32)) {
    install_signal_handler(SIG_INT, handler);
    install_signal_handler(SIG_TERM, handler);
}

/// Setup daemon signals (SIGINT/SIGTERM for stop, SIGHUP for reload).
pub fn setup_daemon_signals(on_stop: extern "C" fn(i32), on_hup: extern "C" fn(i32)) {
    install_signal_handler(SIG_INT, on_stop);
    install_signal_handler(SIG_TERM, on_stop);
    install_signal_handler(SIG_HUP, on_hup);
}

/// Self-pipe wake mechanism for background daemons.
pub struct WakePipe {
    read_fd: i32,
    write_fd: i32,
}

impl WakePipe {
    pub fn new() -> Result<Self> {
        #[cfg(unix)]
        {
            let mut pipe = [0 as libc::c_int; 2];
            unsafe {
                if libc::pipe(pipe.as_mut_ptr()) != 0 {
                    return Err(PlatformError::Io("failed to create pipe".to_string()));
                }
                libc::fcntl(pipe[1], libc::F_SETFL, libc::O_NONBLOCK);
            }
            Ok(Self { read_fd: pipe[0], write_fd: pipe[1] })
        }
        #[cfg(not(unix))]
        {
            Err(PlatformError::unsupported("wake_pipe"))
        }
    }

    pub fn write_fd(&self) -> i32 {
        self.write_fd
    }

    /// Signal-safe wake write.
    pub fn wake_async_signal_safe(fd: i32) {
        #[cfg(unix)]
        if fd >= 0 {
            unsafe {
                libc::write(fd, b"x".as_ptr() as *const libc::c_void, 1);
            }
        }
        #[cfg(not(unix))]
        let _ = fd;
    }

    /// Block until woken by SIGHUP (or write on the pipe).
    pub fn wait_for_wake(&self) {
        #[cfg(unix)]
        {
            let mut pfd = libc::pollfd { fd: self.read_fd, events: libc::POLLIN, revents: 0 };
            unsafe {
                if libc::poll(&mut pfd, 1, -1) > 0 {
                    let mut buf = [0u8; 64];
                    libc::read(self.read_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
                }
            }
        }
    }
}

impl Drop for WakePipe {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            if self.read_fd >= 0 {
                libc::close(self.read_fd);
            }
            if self.write_fd >= 0 {
                libc::close(self.write_fd);
            }
        }
    }
}

#[cfg(test)]
#[path = "test_signal.rs"]
mod tests;
