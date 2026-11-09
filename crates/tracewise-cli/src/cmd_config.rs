//! `tracewise config get|set`: settings by command instead of editing `.tracewiseconfig` by hand. See docs/configuration.md.
use std::fs;
use std::path::{Path, PathBuf};

/// (key, default, allowed values or a description, env variable that overrides it)
const KEYS: [(&str, &str, &str, Option<&str>); 6] = [
    ("profile.name", "core", "core | full", Some("TRACEWISE_PROFILE")),
    ("reasons.mode", "warn", "off | warn | enforce", Some("TRACEWISE_REASONS")),
    ("retention.commands", "30d", "off | <n>d | <n>w", None),
    ("storage.max_file", "5mb", "<n>kb | <n>mb", None),
    ("storage.secrets", "hash", "hash | store", None),
    ("durability.level", "full", "full | normal", None),
];

fn valid(key: &str, v: &str) -> bool {
    let num = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    match key {
        "profile.name" => matches!(v, "core" | "full"),
        "reasons.mode" => matches!(v, "off" | "warn" | "enforce"),
        "retention.commands" => v == "off" || ((v.ends_with('d') || v.ends_with('w')) && num(&v[..v.len() - 1])),
        "storage.max_file" => (v.ends_with("kb") || v.ends_with("mb")) && num(&v[..v.len() - 2]),
        "storage.secrets" => matches!(v, "hash" | "store"),
        "durability.level" => matches!(v, "full" | "normal"),
        _ => false,
    }
}

/// The value of `section.key` in a config text (quotes and trailing comments removed).
pub(crate) fn read_value(text: &str, key: &str) -> Option<String> {
    let (sec, name) = key.split_once('.')?;
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            inside = line[1..line.len() - 1].trim() == sec;
        } else if inside {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == name {
                    return Some(v.split('#').next().unwrap_or("").trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

/// The text with `section.key = "value"` set: the line is replaced, or added to its section (made if missing). Other lines stay.
pub(crate) fn set_value(text: &str, key: &str, value: &str) -> String {
    let (sec, name) = key.split_once('.').unwrap_or(("", key));
    let new_line = format!("{name} = \"{value}\"");
    let mut out: Vec<String> = Vec::new();
    let (mut inside, mut done, mut section_seen) = (false, false, false);
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            if inside && !done {
                out.push(new_line.clone());
                done = true;
            }
            inside = t[1..t.len() - 1].trim() == sec;
            section_seen |= inside;
        } else if inside && !done && t.split_once('=').is_some_and(|(k, _)| k.trim() == name) {
            out.push(new_line.clone());
            done = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !done {
        if !section_seen {
            if out.last().is_some_and(|l| !l.is_empty()) {
                out.push(String::new());
            }
            out.push(format!("[{sec}]"));
        }
        out.push(new_line);
    }
    out.join("\n") + "\n"
}

fn files(project: Option<&Path>) -> (Option<PathBuf>, PathBuf) {
    (project.map(|p| p.join(".tracewiseconfig")), tracewise_core::get_tracewise_home().join("config.toml"))
}

/// (value, where it came from) for one key.
fn lookup(key: &str, project: Option<&Path>) -> Option<(String, &'static str)> {
    let (_, default, _, env) = *KEYS.iter().find(|k| k.0 == key)?;
    if let Some(v) = env.and_then(|e| std::env::var(e).ok()).filter(|v| valid(key, v)) {
        return Some((v, "env"));
    }
    let (proj, glob) = files(project);
    let read = |p: &Path| fs::read_to_string(p).ok().and_then(|t| read_value(&t, key));
    if let Some(v) = proj.as_deref().and_then(read) {
        return Some((v, "project"));
    }
    if let Some(v) = read(&glob) {
        return Some((v, "global"));
    }
    Some((default.to_string(), "default"))
}

fn fail(msg: &str) -> ! {
    eprintln!("tracewise: {msg}");
    std::process::exit(1);
}

pub(crate) fn get(json: bool, key: Option<String>, project: Option<&Path>) {
    let keys: Vec<&str> = match &key {
        Some(k) if KEYS.iter().any(|x| x.0 == k) => vec![k.as_str()],
        Some(k) => fail(&format!("unknown setting '{k}'; known: {}", KEYS.map(|x| x.0).join(", "))),
        None => KEYS.iter().map(|x| x.0).collect(),
    };
    let rows: Vec<serde_json::Value> =
        keys.iter().filter_map(|k| lookup(k, project).map(|(v, s)| serde_json::json!({ "key": k, "value": v, "source": s }))).collect();
    if json {
        println!("{}", if key.is_some() { rows[0].clone() } else { serde_json::Value::Array(rows) });
    } else {
        for r in &rows {
            println!(
                "{} = {}   ({})",
                r["key"].as_str().unwrap_or(""),
                r["value"].as_str().unwrap_or(""),
                r["source"].as_str().unwrap_or("")
            );
        }
    }
}

pub(crate) fn set(json: bool, key: &str, value: &str, global: bool, project: Option<&Path>) {
    // a setting an agent could change would let it switch off the checks that watch it
    if tracewise_core::model::Actor::from_env().is_some_and(|a| a.kind == "agent") {
        fail("refused: agents cannot change Tracewise's settings");
    }
    let Some((_, _, allowed, _)) = KEYS.iter().find(|k| k.0 == key) else {
        fail(&format!("unknown setting '{key}'; known: {}", KEYS.map(|x| x.0).join(", ")))
    };
    if !valid(key, value) {
        fail(&format!("'{value}' is not valid for {key} ({allowed})"));
    }
    let (proj, glob) = files(project);
    let target =
        if global { glob } else { proj.unwrap_or_else(|| fail("not in a repository; use --global for the settings of all repositories")) };
    let text = fs::read_to_string(&target).unwrap_or_default();
    if let Some(dir) = target.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Err(e) = fs::write(&target, set_value(&text, key, value)) {
        fail(&format!("cannot write {}: {e}", target.display()));
    }
    if json {
        println!("{}", serde_json::json!({ "key": key, "value": value, "file": target }));
    } else {
        println!("{key} = {value}   ({})", target.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_sets_keys_without_touching_other_lines() {
        let t = "# mine\n[share]\nremote = \"x\"\n\n[reasons]\nmode = \"warn\"   # why\n";
        assert_eq!(read_value(t, "reasons.mode").as_deref(), Some("warn"));
        assert_eq!(read_value(t, "share.mode"), None);
        let s = set_value(t, "reasons.mode", "enforce");
        assert_eq!(read_value(&s, "reasons.mode").as_deref(), Some("enforce"));
        assert!(s.contains("# mine") && s.contains("remote = \"x\""));
        let s2 = set_value(&s, "storage.secrets", "store");
        assert_eq!(read_value(&s2, "storage.secrets").as_deref(), Some("store"));
        assert_eq!(read_value(&s2, "reasons.mode").as_deref(), Some("enforce"));
        assert_eq!(set_value("", "durability.level", "normal"), "[durability]\nlevel = \"normal\"\n");
        // a key added to a section that is followed by another section stays in its own section
        let s3 = set_value("[reasons]\n[share]\nremote = \"x\"\n", "reasons.mode", "off");
        assert_eq!(read_value(&s3, "reasons.mode").as_deref(), Some("off"));
        assert_eq!(read_value(&s3, "share.remote").as_deref(), Some("x"));
    }

    #[test]
    fn values_are_checked() {
        assert!(valid("reasons.mode", "enforce") && !valid("reasons.mode", "loud"));
        assert!(valid("retention.commands", "30d") && valid("retention.commands", "off") && !valid("retention.commands", "d"));
        assert!(valid("storage.max_file", "5mb") && !valid("storage.max_file", "5"));
        assert!(!valid("nope.key", "x"));
    }
}
