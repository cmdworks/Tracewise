use std::path::PathBuf;

use crate::{operands, WriteTargets};

pub fn detect_tee_targets(args: &[String]) -> WriteTargets {
    let files: Vec<PathBuf> = operands(args).into_iter().map(PathBuf::from).collect();
    if files.is_empty() {
        WriteTargets::None
    } else {
        WriteTargets::Paths(files)
    }
}

/// Where `sources` end up: inside `dest` when it is a folder, else `dest` itself.
fn into_dir(sources: &[PathBuf], dest: PathBuf) -> Vec<PathBuf> {
    if dest.is_dir() {
        sources.iter().filter_map(|s| s.file_name().map(|f| dest.join(f))).collect()
    } else {
        vec![dest]
    }
}

pub fn detect_mv_targets(args: &[String]) -> WriteTargets {
    let non_flags: Vec<&String> = operands(args).into_iter().collect();
    if non_flags.len() < 2 {
        return WriteTargets::AnyInCwd;
    }
    let dest = PathBuf::from(non_flags.last().unwrap());
    let sources: Vec<PathBuf> = non_flags[..non_flags.len() - 1].iter().map(|s| PathBuf::from(*s)).collect();
    let mut targets = into_dir(&sources, dest);
    targets.extend(sources);
    WriteTargets::Paths(targets)
}

pub fn detect_cp_targets(args: &[String]) -> WriteTargets {
    let non_flags: Vec<&String> = operands(args).into_iter().collect();
    if non_flags.len() < 2 {
        return WriteTargets::AnyInCwd;
    }
    let dest = PathBuf::from(non_flags.last().unwrap());
    let sources: Vec<PathBuf> = non_flags[..non_flags.len() - 1].iter().map(|s| PathBuf::from(*s)).collect();
    WriteTargets::Paths(into_dir(&sources, dest))
}

pub fn detect_ln_targets(args: &[String]) -> WriteTargets {
    let non_flags: Vec<&String> = operands(args).into_iter().collect();
    if non_flags.is_empty() {
        return WriteTargets::AnyInCwd;
    }
    if non_flags.len() == 1 {
        let s = PathBuf::from(non_flags[0]);
        if let Some(fname) = s.file_name() {
            return WriteTargets::Paths(vec![PathBuf::from(fname)]);
        } else {
            return WriteTargets::AnyInCwd;
        }
    }
    let dest = PathBuf::from(non_flags.last().unwrap());
    let sources: Vec<PathBuf> = non_flags[..non_flags.len() - 1].iter().map(|s| PathBuf::from(*s)).collect();
    WriteTargets::Paths(into_dir(&sources, dest))
}

pub fn detect_rm_targets(args: &[String]) -> WriteTargets {
    let non_flags: Vec<PathBuf> = operands(args).into_iter().map(PathBuf::from).collect();
    if non_flags.is_empty() {
        WriteTargets::AnyInCwd
    } else {
        WriteTargets::Paths(non_flags)
    }
}

pub fn detect_truncate_targets(args: &[String]) -> WriteTargets {
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--" {
            files.extend(args[i + 1..].iter().map(PathBuf::from));
            break;
        }
        if a == "-s" || a == "-r" || a == "--size" || a == "--reference" {
            i += 2;
            continue;
        }
        if a.starts_with("-s") || a.starts_with("-r") {
            i += 1;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        files.push(PathBuf::from(a));
        i += 1;
    }
    if files.is_empty() {
        WriteTargets::AnyInCwd
    } else {
        WriteTargets::Paths(files)
    }
}

pub fn detect_chflags_targets(args: &[String]) -> WriteTargets {
    // First operand is the flag spec, the rest are files.
    let files: Vec<PathBuf> = operands(args).into_iter().skip(1).map(PathBuf::from).collect();
    if files.is_empty() {
        WriteTargets::None
    } else {
        WriteTargets::Paths(files)
    }
}

pub fn detect_xattr_targets(args: &[String]) -> WriteTargets {
    let flags: String = args.iter().filter(|a| a.starts_with('-') && !a.starts_with("--")).flat_map(|a| a[1..].chars()).collect();
    let skip = if flags.contains('w') {
        2
    } else if flags.contains('d') {
        1
    } else if flags.contains('c') {
        0
    } else {
        return WriteTargets::None;
    };
    let files: Vec<PathBuf> = operands(args).into_iter().skip(skip).map(PathBuf::from).collect();
    if files.is_empty() {
        WriteTargets::None
    } else {
        WriteTargets::Paths(files)
    }
}

pub fn detect_touch_targets(args: &[String]) -> WriteTargets {
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--" {
            files.extend(args[i + 1..].iter().map(PathBuf::from));
            break;
        }
        if a == "-d" || a == "-t" || a == "-r" || a == "--date" || a == "--reference" {
            i += 2;
            continue;
        }
        if a.starts_with("-d") || a.starts_with("-t") || a.starts_with("-r") {
            i += 1;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        files.push(PathBuf::from(a));
        i += 1;
    }
    if files.is_empty() {
        WriteTargets::AnyInCwd
    } else {
        WriteTargets::Paths(files)
    }
}

/// `cp` and `mv`: the source operands and the destination (the last operand). None for any other tool or fewer
/// than two operands.
pub fn copy_operands(tool: &str, args: &[String]) -> Option<(Vec<String>, String)> {
    if tool != "cp" && tool != "mv" {
        return None;
    }
    let ops = operands(args);
    if ops.len() < 2 {
        return None;
    }
    let (dest, sources) = ops.split_last()?;
    Some((sources.iter().map(|s| s.to_string()).collect(), dest.to_string()))
}

pub fn detect_fs_targets(tool: &str, args: &[String]) -> Option<WriteTargets> {
    match tool {
        "tee" => Some(detect_tee_targets(args)),
        "mv" => Some(detect_mv_targets(args)),
        "cp" => Some(detect_cp_targets(args)),
        "ln" => Some(detect_ln_targets(args)),
        "rm" => Some(detect_rm_targets(args)),
        "truncate" => Some(detect_truncate_targets(args)),
        "chflags" => Some(detect_chflags_targets(args)),
        "xattr" => Some(detect_xattr_targets(args)),
        "touch" => Some(detect_touch_targets(args)),
        _ => None,
    }
}
