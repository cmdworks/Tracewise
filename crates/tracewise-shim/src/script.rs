use std::path::PathBuf;

use crate::WriteTargets;

pub fn detect_perl_targets(args: &[String]) -> WriteTargets {
    let mut in_place = false;
    let mut backup_suffix = String::new();
    let mut input_files = Vec::new();
    let mut i = 0;
    let mut past_script = false;

    while i < args.len() {
        let a = &args[i];
        if a == "--" {
            i += 1;
            while i < args.len() {
                input_files.push(args[i].clone());
                i += 1;
            }
            break;
        }
        if a == "-i" {
            in_place = true;
            if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                let next = &args[i + 1];
                if next.starts_with('.') || next.is_empty() {
                    backup_suffix = next.clone();
                    i += 2;
                    continue;
                }
            }
            i += 1;
            continue;
        }
        if let Some(suf) = a.strip_prefix("-i") {
            in_place = true;
            backup_suffix = suf.to_string();
            i += 1;
            continue;
        }
        if a.starts_with('-') && !a.starts_with("--") && a.contains('i') {
            in_place = true;
            let chars: Vec<char> = a[1..].chars().collect();
            if let Some(pos) = chars.iter().position(|&c| c == 'i') {
                let after: String = chars[pos + 1..].iter().collect();
                if !after.is_empty() {
                    backup_suffix = after;
                }
            }
            i += 1;
            continue;
        }
        if a == "-e" || a == "-E" {
            past_script = true;
            i += 2;
            continue;
        }
        if a.starts_with("-e") || a.starts_with("-E") {
            past_script = true;
            i += 1;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        if !past_script {
            past_script = true;
            i += 1;
            continue;
        }
        input_files.push(a.clone());
        i += 1;
    }

    if !in_place {
        return WriteTargets::None;
    }
    if input_files.is_empty() {
        return WriteTargets::AnyInCwd;
    }
    let mut targets = Vec::new();
    for f in input_files {
        targets.push(PathBuf::from(&f));
        if !backup_suffix.is_empty() {
            targets.push(PathBuf::from(format!("{}{}", f, backup_suffix)));
        }
    }
    WriteTargets::Paths(targets)
}

pub fn detect_awk_targets(args: &[String]) -> WriteTargets {
    let mut in_place = false;
    let mut input_files = Vec::new();
    let mut past_script = false;
    let mut i = 0;

    while i < args.len() {
        let a = &args[i];
        if a == "--" {
            i += 1;
            while i < args.len() {
                input_files.push(args[i].clone());
                i += 1;
            }
            break;
        }
        if a == "-i" {
            if i + 1 < args.len() && args[i + 1] == "inplace" {
                in_place = true;
                i += 2;
                continue;
            }
        } else if a == "-iinplace" || a == "--inplace" {
            in_place = true;
            i += 1;
            continue;
        }
        if a == "-f" || a == "-e" {
            past_script = true;
            i += 2;
            continue;
        }
        if a.starts_with("-f") || a.starts_with("-e") {
            past_script = true;
            i += 1;
            continue;
        }
        if a == "-F" || a == "-v" {
            i += 2;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        if !past_script {
            past_script = true;
            i += 1;
            continue;
        }
        input_files.push(a.clone());
        i += 1;
    }

    if !in_place {
        return WriteTargets::None;
    }
    if input_files.is_empty() {
        return WriteTargets::AnyInCwd;
    }
    WriteTargets::Paths(input_files.into_iter().map(PathBuf::from).collect())
}
