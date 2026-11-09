use std::fs;
use std::path::PathBuf;

use crate::WriteTargets;

pub fn parse_patch_targets(patch_text: &str, strip_count: usize) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let mut last_old_path: Option<String> = None;
    for line in patch_text.lines() {
        if let Some(rest) = line.strip_prefix("--- ") {
            let mut raw = rest;
            if let Some(pos) = raw.find('\t') {
                raw = &raw[..pos];
            }
            last_old_path = Some(raw.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            let mut raw = rest;
            if let Some(pos) = raw.find('\t') {
                raw = &raw[..pos];
            }
            let trimmed = raw.trim();
            let target_str = if trimmed == "/dev/null" { last_old_path.take() } else { Some(trimmed.to_string()) };
            if let Some(path) = target_str {
                if path != "/dev/null" && !path.is_empty() {
                    let parts: Vec<&str> = path.split('/').collect();
                    let final_path = if strip_count < parts.len() {
                        parts[strip_count..].join("/")
                    } else {
                        parts.last().unwrap_or(&path.as_str()).to_string()
                    };
                    if !final_path.is_empty() {
                        let p = PathBuf::from(final_path);
                        if !targets.contains(&p) {
                            targets.push(p);
                        }
                    }
                }
            }
        }
    }
    targets
}

pub fn detect_patch_targets(args: &[String]) -> WriteTargets {
    let mut patch_file: Option<PathBuf> = None;
    let mut strip_count = 0;
    let mut positional = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let a = &args[i];
        if (a == "-i" || a == "--input") && i + 1 < args.len() {
            patch_file = Some(PathBuf::from(&args[i + 1]));
            i += 2;
            continue;
        }
        if let Some(f) = a.strip_prefix("-i") {
            patch_file = Some(PathBuf::from(f));
            i += 1;
            continue;
        }
        if a == "-p" && i + 1 < args.len() {
            strip_count = args[i + 1].parse().unwrap_or(0);
            i += 2;
            continue;
        }
        if let Some(num) = a.strip_prefix("-p") {
            strip_count = num.parse().unwrap_or(0);
            i += 1;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        positional.push(a.clone());
        i += 1;
    }

    if let Some(pf) = patch_file {
        if let Ok(content) = fs::read_to_string(&pf) {
            let targets = parse_patch_targets(&content, strip_count);
            if !targets.is_empty() {
                return WriteTargets::Paths(targets);
            }
        }
    } else if positional.len() >= 2 {
        let pf = PathBuf::from(&positional[1]);
        if let Ok(content) = fs::read_to_string(&pf) {
            let targets = parse_patch_targets(&content, strip_count);
            if !targets.is_empty() {
                return WriteTargets::Paths(targets);
            }
        }
        return WriteTargets::Paths(vec![PathBuf::from(&positional[0])]);
    } else if positional.len() == 1 {
        return WriteTargets::Paths(vec![PathBuf::from(&positional[0])]);
    }

    WriteTargets::AnyInCwd
}
