//! Which files a shimmed command (sed, perl, awk, gawk, tee, mv, cp, rm, truncate, patch, ln, touch) will write.
//! std only: shared by the tiny `tracewise-shim` entry binary and the full `tracewise`, so there is one classifier.
use std::path::{Path, PathBuf};

pub mod fs;
pub mod patch;
pub mod script;
pub mod sed;

#[cfg(test)]
mod tests;

pub use fs::copy_operands;
pub use patch::parse_patch_targets;
pub use sed::{detect_sed_targets, extract_sed_script_w_targets, find_unescaped_delim};

pub fn is_tracewise_bin_name(name: &str) -> bool {
    matches!(
        name,
        "tracewise"
            | "tracewise.exe"
            | "tracewise-git"
            | "tracewise-git.exe"
            | "tracewise-shim"
            | "tracewise-shim.exe"
    )
}

pub fn is_tracewise_bin(path: &Path, self_exe: Option<&Path>) -> bool {
    let check_path = |p: &Path| {
        p.file_name().and_then(|n| n.to_str()).is_some_and(is_tracewise_bin_name) || p.components().any(|c| c.as_os_str() == ".tracewise")
    };
    if let Ok(canon) = path.canonicalize() {
        if self_exe == Some(canon.as_path()) || check_path(&canon) {
            return true;
        }
    }
    std::fs::read_link(path).is_ok_and(|target| check_path(&target))
}

/// The real `tool`: `TRACEWISE_REAL_<TOOL>`, else the next match on `path_var` after `shim_dir` that is not a Tracewise binary,
/// else `/usr/bin` or `/bin`. One copy for the tiny shim and for `tracewise`.
pub fn find_real_tool_inner(tool: &str, path_var: Option<&std::ffi::OsStr>, shim_dir: Option<&Path>) -> PathBuf {
    if let Some(p) = std::env::var_os(format!("TRACEWISE_REAL_{}", tool.to_uppercase())).map(PathBuf::from).filter(|p| p.is_file()) {
        return p;
    }
    let mut self_exe: Option<Option<PathBuf>> = None;
    let mut past_shim_dir = shim_dir.is_none();
    for dir in path_var.map(|p| std::env::split_paths(p).collect::<Vec<_>>()).unwrap_or_default() {
        if !past_shim_dir {
            past_shim_dir = Some(dir.as_path()) == shim_dir;
            continue;
        }
        let candidate = dir.join(tool);
        if let Ok(meta) = std::fs::symlink_metadata(&candidate) {
            // only a link can point back at Tracewise; resolving our own path is done once, lazily
            let me = || std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
            if meta.is_symlink() && is_tracewise_bin(&candidate, self_exe.get_or_insert_with(me).as_deref()) {
                continue;
            }
            return candidate;
        }
    }
    [format!("/usr/bin/{tool}"), format!("/bin/{tool}")]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(format!("/usr/bin/{tool}")))
}

pub fn find_real_tool(tool: &str, shim_dir: Option<&Path>) -> PathBuf {
    find_real_tool_inner(tool, std::env::var_os("PATH").as_deref(), shim_dir)
}

/// When any argument is not UTF-8, run the real tool with the original bytes and never return; a lossy copy would change the
/// command. No record is made, as when Tracewise is not found. Returns when every argument is UTF-8.
pub fn exec_if_non_utf8(tool: &str, args: &[std::ffi::OsString], shim_dir: Option<&Path>) {
    if args.iter().all(|a| a.to_str().is_some()) {
        return;
    }
    tracewise_platform::process::fast_exec(&find_real_tool(tool, shim_dir), args, Some(tool), Some(("TRACEWISE_SHIM_DEPTH", "1")), None);
}

/// The loop guard `TRACEWISE_SHIM_DEPTH` counts only when Tracewise itself started this process: the parent is a Tracewise binary that lives
/// in the same folder as this one. Set by anyone else (an agent, a script, a copy named `tracewise` elsewhere), it is ignored.
pub fn shim_guard_applies(depth_set: bool, parent_exe: Option<&str>, self_exe: Option<&Path>) -> bool {
    let dir_of = |p: &Path| p.canonicalize().ok().and_then(|c| c.parent().map(Path::to_path_buf));
    depth_set
        && parent_exe.map(Path::new).is_some_and(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(is_tracewise_bin_name)
                && dir_of(p).is_some()
                && dir_of(p) == self_exe.and_then(dir_of)
        })
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WriteTargets {
    None,
    Paths(Vec<PathBuf>),
    AnyInCwd,
}

/// The non-option arguments of a command: everything not starting with `-`, and everything after `--`.
pub(crate) fn operands(args: &[String]) -> Vec<&String> {
    let mut out = Vec::new();
    let mut after_dashes = false;
    for a in args {
        if after_dashes {
            out.push(a);
        } else if a == "--" {
            after_dashes = true;
        } else if !a.starts_with('-') {
            out.push(a);
        }
    }
    out
}

pub fn detect_write_targets(tool: &str, args: &[String]) -> WriteTargets {
    match tool {
        "sed" => sed::detect_sed_targets(args),
        "perl" => script::detect_perl_targets(args),
        "awk" | "gawk" => script::detect_awk_targets(args),
        "patch" => patch::detect_patch_targets(args),
        "tee" | "mv" | "cp" | "ln" | "rm" | "truncate" | "chflags" | "xattr" | "touch" => {
            fs::detect_fs_targets(tool, args).unwrap_or(WriteTargets::AnyInCwd)
        }
        _ => WriteTargets::AnyInCwd,
    }
}

pub fn is_read_only(tool: &str, args: &[String]) -> bool {
    detect_write_targets(tool, args) == WriteTargets::None
}
