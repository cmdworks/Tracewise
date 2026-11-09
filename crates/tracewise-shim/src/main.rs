use std::path::{Path, PathBuf};
use std::process;

#[cfg(test)]
use tracewise_shim::find_real_tool_inner;
use tracewise_shim::{exec_if_non_utf8, find_real_tool, is_read_only, shim_guard_applies};

pub fn find_trail_bin_inner(self_exe: Option<&Path>, trail_bin_env: Option<&str>, home_env: Option<&str>) -> Option<PathBuf> {
    if let Some(exe) = self_exe {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("trail");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(p) = trail_bin_env {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Some(h) = home_env {
        let candidate = PathBuf::from(h).join(".cargo/bin/trail");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn find_trail_bin() -> Option<PathBuf> {
    let self_exe = std::env::current_exe().ok().map(|p| p.canonicalize().unwrap_or(p));
    let trail_bin = std::env::var("TRAIL_BIN").ok();
    let home = std::env::var("HOME").ok();
    find_trail_bin_inner(self_exe.as_deref(), trail_bin.as_deref(), home.as_deref())
}

fn exec_tool(target: &Path, args: &[String], arg0: Option<&str>, set_depth: bool) -> ! {
    let env_set = if set_depth { Some(("TRAIL_SHIM_DEPTH", "1")) } else { None };
    let env_remove = if !set_depth { Some("TRAIL_SHIM_DEPTH") } else { None };
    tracewise_platform::process::fast_exec(target, args, arg0, env_set, env_remove);
}

fn main() {
    let os_args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let args: Vec<String> = os_args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let prog = if !args.is_empty() { &args[0] } else { "tool" };
    let tool = Path::new(prog).file_name().and_then(|n| n.to_str()).unwrap_or("");

    if tool == "trail-shim" || tool.is_empty() {
        eprintln!("trail-shim: fast tiny dispatcher for command shims");
        process::exit(0);
    }

    exec_if_non_utf8(tool, os_args.get(1..).unwrap_or(&[]), Path::new(prog).parent());
    let tool_args = if args.len() > 1 { &args[1..] } else { &[] };

    // 1. Loop guard: if TRAIL_SHIM_DEPTH is set, exec system tool directly
    let parent = tracewise_platform::process::exe(tracewise_platform::process::parent_pid());
    if shim_guard_applies(std::env::var_os("TRAIL_SHIM_DEPTH").is_some(), parent.as_deref(), std::env::current_exe().ok().as_deref()) {
        let fallback = PathBuf::from(format!("/usr/bin/{}", tool));
        let target = if fallback.is_file() { fallback } else { PathBuf::from(format!("/bin/{}", tool)) };
        exec_tool(&target, tool_args, Some(tool), true);
    }

    let shim_dir = Path::new(prog).parent();
    let real_tool = find_real_tool(tool, shim_dir);

    // 2. Read-only fast path (< 1 ms): exec real tool directly
    if is_read_only(tool, tool_args) {
        exec_tool(&real_tool, tool_args, Some(tool), true);
    }

    // 3. Mutating path: exec trail binary with argv[0] = prog
    if let Some(trail_bin) = find_trail_bin() {
        exec_tool(&trail_bin, tool_args, Some(prog), false);
    }

    // Fallback: if trail not found, fail open
    exec_tool(&real_tool, tool_args, Some(tool), true);
}

#[cfg(test)]
#[path = "test_main.rs"]
mod tests;
