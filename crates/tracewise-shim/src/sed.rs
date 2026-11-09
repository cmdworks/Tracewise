use std::fs;
use std::path::PathBuf;

use crate::WriteTargets;

pub fn find_unescaped_delim(s: &str, delim: char) -> Option<usize> {
    let mut escaped = false;
    for (idx, c) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == delim {
            return Some(idx);
        }
    }
    None
}

pub fn extract_sed_script_w_targets(script: &str) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    for line in script.lines() {
        for cmd in line.split(';') {
            let trimmed = cmd.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Check s/.../.../w file
            if trimmed.starts_with('s') && trimmed.len() > 1 {
                // the delimiter can be any character, so slice after its own length, never at byte 2
                let delim = trimmed.chars().nth(1).unwrap_or('/');
                let rest = &trimmed[1 + delim.len_utf8()..];
                if let Some(idx1) = find_unescaped_delim(rest, delim) {
                    let after_pat = &rest[idx1 + delim.len_utf8()..];
                    if let Some(idx2) = find_unescaped_delim(after_pat, delim) {
                        let flags = &after_pat[idx2 + delim.len_utf8()..];
                        if let Some(pos) = flags.find('w').or_else(|| flags.find('W')) {
                            let file = flags[pos + 1..].trim();
                            if !file.is_empty() {
                                targets.push(PathBuf::from(file));
                                continue;
                            }
                        }
                    }
                }
            }
            // Check [address]w file or [address]W file
            let chars = trimmed.char_indices();
            let mut in_regex = false;
            let regex_delim = '/';
            let mut escaped = false;
            let mut w_pos = None;

            for (idx, c) in chars {
                if escaped {
                    escaped = false;
                    continue;
                }
                if c == '\\' {
                    escaped = true;
                    continue;
                }
                if in_regex {
                    if c == regex_delim {
                        in_regex = false;
                    }
                    continue;
                }
                if c == '/' {
                    in_regex = true;
                    continue;
                }
                if (c == 'w' || c == 'W') && !in_regex {
                    w_pos = Some(idx);
                    break;
                }
                if !c.is_ascii_digit() && c != '$' && c != ',' && c != '!' && !c.is_whitespace() && c != '+' && c != '~' && c != '{' {
                    break;
                }
            }

            if let Some(pos) = w_pos {
                let file = trimmed[pos + 1..].trim();
                if !file.is_empty() {
                    targets.push(PathBuf::from(file));
                }
            }
        }
    }
    targets
}

struct SedParsedArgs {
    in_place: bool,
    backup_suffix: Option<String>,
    /// BSD `sed` takes the argument after a bare `-i` as the suffix even when it is `-e`; GNU does not. Name both.
    bsd_suffix: Option<String>,
    scripts: Vec<String>,
    input_files: Vec<String>,
}

fn parse_sed_long_opt(long: &str, args: &[String], i: &mut usize, scripts: &mut Vec<String>) -> Option<bool> {
    let (name, value) = match long.split_once('=') {
        Some((n, v)) => (n, Some(v)),
        None => (long, None),
    };
    match name {
        "expression" | "file" => {
            let text = match (value, args.get(*i + 1)) {
                (Some(v), _) => Some((v.to_string(), 1)),
                (None, Some(next)) => Some((next.clone(), 2)),
                (None, None) => None,
            };
            let (v, used) = text?;
            // a script file that cannot be read may hold `w` commands: unknown targets, never "no writes"
            scripts.push(if name == "expression" { v } else { fs::read_to_string(&v).ok()? });
            *i += used;
            Some(true)
        }
        "line-length" => {
            *i += if value.is_some() { 1 } else { 2 };
            Some(true)
        }
        "quiet" | "silent" | "regexp-extended" | "separate" | "null-data" | "posix" | "debug" | "sandbox" | "follow-symlinks"
        | "unbuffered" | "binary" | "zero-terminated" | "version" | "help" => {
            *i += 1;
            Some(true)
        }
        _ => None,
    }
}

fn parse_sed_short_flag(arg: &str, args: &[String], i: &mut usize, in_place: &mut bool, backup_suffix: &mut Option<String>) -> bool {
    let chars: Vec<char> = arg[1..].chars().collect();
    if let Some(pos) = chars.iter().position(|&c| c == 'i') {
        *in_place = true;
        let after: String = chars[pos + 1..].iter().collect();
        if !after.is_empty() {
            *backup_suffix = Some(after);
        } else if *i + 1 < args.len() && args[*i + 1].is_empty() {
            *backup_suffix = Some(String::new());
            *i += 2;
            return true;
        } else {
            *backup_suffix = Some(String::new());
        }
        *i += 1;
        return true;
    }
    *i += 1;
    true
}

fn parse_sed_args(args: &[String]) -> Option<SedParsedArgs> {
    let mut in_place = false;
    let mut backup_suffix: Option<String> = None;
    let mut bsd_suffix: Option<String> = None;
    let mut scripts: Vec<String> = Vec::new();
    let mut input_files: Vec<String> = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            i += 1;
            while i < args.len() {
                input_files.push(args[i].clone());
                i += 1;
            }
            break;
        }
        if arg == "-i" {
            in_place = true;
            if i + 1 < args.len() {
                let next = &args[i + 1];
                if next.is_empty() {
                    backup_suffix = Some(String::new());
                    i += 2;
                    continue;
                } else if next.starts_with('.') && !next.contains('/') {
                    backup_suffix = Some(next.clone());
                    i += 2;
                    continue;
                }
            }
            backup_suffix = Some(String::new());
            bsd_suffix = args.get(i + 1).filter(|n| n.starts_with('-') && n.len() > 1).cloned();
            i += 1;
            continue;
        }
        if let Some(suf) = arg.strip_prefix("-i") {
            in_place = true;
            backup_suffix = Some(suf.to_string());
            i += 1;
            continue;
        }
        if arg == "--in-place" {
            in_place = true;
            backup_suffix = Some(String::new());
            i += 1;
            continue;
        }
        if let Some(suf) = arg.strip_prefix("--in-place=") {
            in_place = true;
            backup_suffix = Some(suf.to_string());
            i += 1;
            continue;
        }
        if let Some(long) = arg.strip_prefix("--") {
            parse_sed_long_opt(long, args, &mut i, &mut scripts)?;
            continue;
        }
        if arg == "-e" {
            if i + 1 < args.len() {
                scripts.push(args[i + 1].clone());
                i += 2;
                continue;
            } else {
                return None;
            }
        }
        if let Some(script) = arg.strip_prefix("-e") {
            scripts.push(script.to_string());
            i += 1;
            continue;
        }
        if arg == "-f" {
            if i + 1 < args.len() {
                scripts.push(fs::read_to_string(&args[i + 1]).ok()?);
                i += 2;
                continue;
            } else {
                return None;
            }
        }
        if let Some(script_file) = arg.strip_prefix("-f") {
            scripts.push(fs::read_to_string(script_file).ok()?);
            i += 1;
            continue;
        }
        if arg.starts_with('-') && arg.len() > 1 && !arg.starts_with("--") {
            parse_sed_short_flag(arg, args, &mut i, &mut in_place, &mut backup_suffix);
            continue;
        }
        input_files.push(arg.clone());
        i += 1;
    }

    Some(SedParsedArgs { in_place, backup_suffix, bsd_suffix, scripts, input_files })
}

pub fn detect_sed_targets(args: &[String]) -> WriteTargets {
    let Some(mut parsed) = parse_sed_args(args) else {
        return WriteTargets::AnyInCwd;
    };

    if parsed.scripts.is_empty() && !parsed.input_files.is_empty() {
        let script = parsed.input_files.remove(0);
        parsed.scripts.push(script);
    }

    let mut w_targets = Vec::new();
    for script in &parsed.scripts {
        w_targets.extend(extract_sed_script_w_targets(script));
    }

    if !parsed.in_place && w_targets.is_empty() {
        return WriteTargets::None;
    }

    let mut all_targets = Vec::new();
    if parsed.in_place {
        if parsed.input_files.is_empty() {
            return WriteTargets::AnyInCwd;
        }
        for file in &parsed.input_files {
            let p = PathBuf::from(file);
            all_targets.push(p);
            if let Some(suf) = &parsed.bsd_suffix {
                all_targets.push(PathBuf::from(format!("{file}{suf}")));
            }
            if let Some(ref suf) = parsed.backup_suffix {
                if !suf.is_empty() {
                    all_targets.push(PathBuf::from(format!("{}{}", file, suf)));
                }
            }
        }
    }
    all_targets.extend(w_targets);
    WriteTargets::Paths(all_targets)
}
