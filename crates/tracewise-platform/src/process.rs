//! Operating system process inspection routines (macOS sysctl and proc_pidinfo, Linux /proc, Windows stubs).
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessEntry {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub path: String,
}

#[cfg(target_os = "macos")]
extern "C" {
    fn proc_pidpath(pid: libc::c_int, buffer: *mut libc::c_void, buffersize: u32) -> libc::c_int;
    fn proc_name(pid: libc::c_int, buffer: *mut libc::c_void, buffersize: u32) -> libc::c_int;
}

/// Retrieve the parent process id of the current process.
pub fn parent_pid() -> u32 {
    #[cfg(unix)]
    unsafe {
        libc::getppid() as u32
    }
    #[cfg(not(unix))]
    0
}

/// Retrieve process command line arguments.
pub fn get_proc_args(pid: u32) -> Option<Vec<String>> {
    #[cfg(target_os = "macos")]
    {
        let mut mib = [libc::CTL_KERN, 49 /* KERN_PROCARGS2 */, pid as libc::c_int];
        let mut size: libc::size_t = 0;
        let ret = unsafe { libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &mut size, std::ptr::null_mut(), 0) };
        if ret != 0 || size <= 4 {
            return None;
        }
        let mut buf = vec![0u8; size];
        let ret = unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr() as *mut libc::c_void, &mut size, std::ptr::null_mut(), 0) };
        if ret != 0 || size <= 4 {
            return None;
        }
        let argc = i32::from_ne_bytes(buf[..4].try_into().ok()?) as usize;
        let mut offset = 4;
        while offset < size && buf[offset] != 0 {
            offset += 1;
        }
        while offset < size && buf[offset] == 0 {
            offset += 1;
        }
        let mut args = Vec::new();
        while offset < size && args.len() < argc {
            let start = offset;
            while offset < size && buf[offset] != 0 {
                offset += 1;
            }
            let arg = String::from_utf8_lossy(&buf[start..offset]).to_string();
            args.push(arg);
            if offset < size && buf[offset] == 0 {
                offset += 1;
            }
        }
        Some(args)
    }

    #[cfg(target_os = "linux")]
    {
        let cmdline_path = format!("/proc/{pid}/cmdline");
        let content = std::fs::read(&cmdline_path).ok()?;
        let args =
            content.split(|&b| b == 0).filter(|chunk| !chunk.is_empty()).map(|chunk| String::from_utf8_lossy(chunk).to_string()).collect();
        Some(args)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

/// Retrieve the parent process id (ppid) for a given pid.
pub fn get_ppid(pid: u32) -> Option<u32> {
    #[cfg(target_os = "macos")]
    {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid as libc::c_int];
        let mut kinfo = [0u8; 648];
        let mut size = std::mem::size_of_val(&kinfo);
        let ret = unsafe { libc::sysctl(mib.as_mut_ptr(), 4, kinfo.as_mut_ptr() as *mut libc::c_void, &mut size, std::ptr::null_mut(), 0) };
        if ret == 0 && size >= 576 {
            let ppid = i32::from_ne_bytes(kinfo[560..564].try_into().ok()?) as u32;
            return Some(ppid);
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        let stat_path = format!("/proc/{pid}/stat");
        let content = std::fs::read_to_string(&stat_path).ok()?;
        // /proc/<pid>/stat format: pid (name) state ppid ...
        let rparen = content.rfind(')')?;
        let rest = content[rparen + 1..].trim();
        let mut parts = rest.split_whitespace();
        let _state = parts.next()?;
        let ppid_str = parts.next()?;
        ppid_str.parse().ok()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

/// Retrieve the executable name of a process.
pub fn get_proc_name(pid: u32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let mut buf = [0u8; 1024];
        let ret = unsafe { proc_name(pid as libc::c_int, buf.as_mut_ptr() as *mut libc::c_void, 1024) };
        if ret > 0 {
            let len = ret as usize;
            return Some(String::from_utf8_lossy(&buf[..len]).trim_end_matches('\0').to_string());
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        let comm_path = format!("/proc/{pid}/comm");
        std::fs::read_to_string(&comm_path).ok().map(|s| s.trim().to_string())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

/// Retrieve the full executable filesystem path of a process.
pub fn get_proc_path(pid: u32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let mut buf = [0u8; 4096];
        let ret = unsafe { proc_pidpath(pid as libc::c_int, buf.as_mut_ptr() as *mut libc::c_void, 4096) };
        if ret > 0 {
            let len = ret as usize;
            return Some(String::from_utf8_lossy(&buf[..len]).trim_end_matches('\0').to_string());
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        let exe_link = format!("/proc/{pid}/exe");
        std::fs::read_link(&exe_link).ok().map(|p| p.to_string_lossy().to_string())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

/// Construct a `ProcessEntry` for a given pid.
pub fn get_process_entry(pid: u32) -> Option<ProcessEntry> {
    let ppid = get_ppid(pid)?;
    let path = get_proc_path(pid).unwrap_or_default();
    let name = get_proc_name(pid).unwrap_or_else(|| {
        if !path.is_empty() {
            Path::new(&path).file_name().and_then(|f| f.to_str()).unwrap_or("unknown").to_string()
        } else {
            "unknown".to_string()
        }
    });

    Some(ProcessEntry { pid, ppid, name, path })
}

/// Build the full process chain starting at `start_pid` up to root (pid 1 or 0).
pub fn process_chain(start_pid: u32) -> Vec<ProcessEntry> {
    let mut chain = Vec::new();
    let mut curr = start_pid;
    let mut visited = HashSet::new();

    while !visited.contains(&curr) && visited.len() < 128 {
        visited.insert(curr);
        let entry = match get_process_entry(curr) {
            Some(e) => e,
            None => break,
        };
        let next_pid = entry.ppid;
        let is_root = curr == 0 || curr == 1;
        chain.push(entry);

        if is_root || next_pid == curr {
            break;
        }
        curr = next_pid;
    }

    chain
}

/// Retrieve the parent PID for `pid`.
pub fn parent(pid: u32) -> Option<u32> {
    get_ppid(pid)
}

/// Retrieve executable path for `pid`.
pub fn exe(pid: u32) -> Option<String> {
    get_proc_path(pid)
}

/// Retrieve process name for `pid`.
pub fn name(pid: u32) -> Option<String> {
    get_proc_name(pid)
}

/// Retrieve process chain starting at `pid`.
pub fn chain(pid: u32) -> Vec<ProcessEntry> {
    process_chain(pid)
}

/// Check if process with `pid` is alive.
pub fn is_alive(pid: u32) -> bool {
    crate::signal::is_process_running(pid as i32)
}

/// Replace current process image with `target` executable.
pub fn fast_exec<S: AsRef<std::ffi::OsStr>>(
    target: &Path,
    args: &[S],
    arg0: Option<&str>,
    env_set: Option<(&str, &str)>,
    env_remove: Option<&str>,
) -> ! {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        if let Ok(c_prog) = CString::new(target.as_os_str().as_encoded_bytes()) {
            let a0_str = arg0.unwrap_or_else(|| target.file_name().and_then(|n| n.to_str()).unwrap_or("tool"));
            let mut c_args = vec![CString::new(a0_str).unwrap_or_else(|_| CString::new("tool").unwrap())];
            let mut ok = true;
            for a in args {
                if let Ok(c) = CString::new(std::os::unix::ffi::OsStrExt::as_bytes(a.as_ref())) {
                    c_args.push(c);
                } else {
                    ok = false;
                    break;
                }
            }
            if ok {
                let c_ptrs: Vec<*const libc::c_char> = c_args.iter().map(|c| c.as_ptr()).chain(std::iter::once(std::ptr::null())).collect();
                unsafe {
                    if let Some((k, v)) = env_set {
                        if let (Ok(ck), Ok(cv)) = (CString::new(k), CString::new(v)) {
                            libc::setenv(ck.as_ptr(), cv.as_ptr(), 1);
                        }
                    }
                    if let Some(k) = env_remove {
                        if let Ok(ck) = CString::new(k) {
                            libc::unsetenv(ck.as_ptr());
                        }
                    }
                    libc::execv(c_prog.as_ptr(), c_ptrs.as_ptr());
                }
            }
        }
        use std::os::unix::process::CommandExt;
        let mut cmd = std::process::Command::new(target);
        cmd.args(args);
        if let Some(a0) = arg0 {
            cmd.arg0(a0);
        }
        if let Some((k, v)) = env_set {
            cmd.env(k, v);
        }
        if let Some(k) = env_remove {
            cmd.env_remove(k);
        }
        let _ = cmd.exec();
    }
    #[cfg(not(unix))]
    {
        let mut cmd = std::process::Command::new(target);
        cmd.args(args);
        if let Some((k, v)) = env_set {
            cmd.env(k, v);
        }
        if let Some(k) = env_remove {
            cmd.env_remove(k);
        }
        let code = cmd.status().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
        std::process::exit(code);
    }
    std::process::exit(1);
}

/// True only for `sed` arguments that cannot write: options from a short allow-list (no `-i`, `-f`, combined `-Ei`, `--file`),
/// and no `w`, `W` or `;` anywhere. Anything else goes to the full classifier.
pub fn sed_args_surely_read_only<'a>(args: impl IntoIterator<Item = &'a [u8]>) -> bool {
    const LONG: &[&[u8]] = &[b"--quiet", b"--silent", b"--regexp-extended", b"--expression", b"--posix", b"--debug"];
    args.into_iter().all(|s| {
        let option_ok = if s.starts_with(b"--") {
            LONG.iter().any(|l| s.starts_with(l))
        } else if s.len() > 1 && s[0] == b'-' {
            s[1..].iter().all(|c| b"nEre".contains(c)) || s.starts_with(b"-e")
        } else {
            true
        };
        option_ok && !s.contains(&b'w') && !s.contains(&b'W') && !s.contains(&b';')
    })
}

/// On macOS, check argv without parsing full arguments, and if it is read-only sed, fast-exec it.
pub fn try_fast_read_only_sed_argv<F>(find_real_sed: F) -> bool
where
    F: FnOnce(Option<&Path>) -> std::path::PathBuf,
{
    if std::env::var_os("TRAIL_SHIM_DEPTH").is_some() {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        let (argv, prog_str) = unsafe {
            extern "C" {
                fn _NSGetArgc() -> *mut libc::c_int;
                fn _NSGetArgv() -> *mut *mut *mut libc::c_char;
            }
            let argc = *_NSGetArgc() as usize;
            let argv_ptr = *_NSGetArgv();
            if argc == 0 || argv_ptr.is_null() {
                return false;
            }
            let slice = std::slice::from_raw_parts_mut(argv_ptr, argc);
            let first = std::ffi::CStr::from_ptr(slice[0]).to_str().unwrap_or("");
            (slice, first)
        };

        let prog_path = Path::new(prog_str);
        let tool = match prog_path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            _ => return false,
        };

        if tool == "sed" {
            let args = argv[1..].iter().take_while(|p| !p.is_null()).map(|&p| unsafe { std::ffi::CStr::from_ptr(p) }.to_bytes());
            if !sed_args_surely_read_only(args) {
                return false;
            }

            let shim_dir = prog_path.parent();
            let real_tool = find_real_sed(shim_dir);

            use std::ffi::CString;
            if let Ok(c_prog) = CString::new(real_tool.as_os_str().as_encoded_bytes()) {
                unsafe {
                    libc::setenv(c"TRAIL_SHIM_DEPTH".as_ptr(), c"1".as_ptr(), 1);
                    static SED_ARG0: &std::ffi::CStr = c"sed";
                    argv[0] = SED_ARG0.as_ptr() as *mut libc::c_char;
                    libc::execv(c_prog.as_ptr(), argv.as_ptr() as *const *const libc::c_char);
                }
            }
        }
    }

    let _ = find_real_sed;
    false
}

#[cfg(test)]
#[path = "test_process.rs"]
mod tests;
