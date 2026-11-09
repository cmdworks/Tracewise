use crate::model::Actor;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonsMode {
    Off,
    Warn,
    Enforce,
}

impl ReasonsMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "warn" => Some(Self::Warn),
            "enforce" => Some(Self::Enforce),
            _ => None,
        }
    }

    /// True if this mode should refuse actions for this actor.
    /// People are never refused regardless of mode.
    pub fn should_enforce(&self, actor: &Actor) -> bool {
        if actor.kind != "agent" {
            return false;
        }
        matches!(self, Self::Enforce)
    }

    /// True if this mode should print warnings/reminders for this actor.
    pub fn should_warn(&self, actor: &Actor) -> bool {
        if actor.kind != "agent" {
            return false;
        }
        matches!(self, Self::Warn | Self::Enforce)
    }
}

/// Parse `.trailconfig` in `root` to find `[reasons] mode = "..."`.
pub fn read_reasons_mode_from_file(root: &Path) -> Option<ReasonsMode> {
    ReasonsMode::parse(&config_value(root, "reasons", "mode")?)
}

/// The value of `key` in `[section]` of `.trailconfig` in `root`, else of the global `$TRAIL_HOME/config.toml`
/// (the project's file wins; quotes and `#` comments removed).
pub fn config_value(root: &Path, section: &str, key: &str) -> Option<String> {
    let project = fs::read_to_string(root.join(".trailconfig")).ok().and_then(|c| value_in(&c, section, key));
    project.or_else(|| fs::read_to_string(crate::get_trail_home().join("config.toml")).ok().and_then(|c| value_in(&c, section, key)))
}

fn value_in(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let mut in_quote = false;
        let mut comment_start = None;
        for (i, c) in line.char_indices() {
            if c == '"' {
                in_quote = !in_quote;
            } else if c == '#' && !in_quote {
                comment_start = Some(i);
                break;
            }
        }
        let effective = match comment_start {
            Some(idx) => &line[..idx],
            None => line,
        }
        .trim();

        if effective.is_empty() {
            continue;
        }

        if effective.starts_with('[') && effective.ends_with(']') {
            in_section = effective[1..effective.len() - 1].trim() == section;
            continue;
        }

        if in_section {
            if let Some((k, v)) = effective.split_once('=') {
                let mut v = v.trim();
                if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
                    v = &v[1..v.len() - 1];
                }
                if k.trim() == key {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Resolve effective reasons mode:
/// 1. `TRAIL_REASONS` env var (`off`, `warn`, `enforce`)
/// 2. `.trailconfig` `[reasons] mode`
/// 3. Default: `warn`
pub fn get_reasons_mode(root: &Path) -> ReasonsMode {
    if let Ok(env_val) = std::env::var("TRAIL_REASONS") {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            if let Some(mode) = ReasonsMode::parse(trimmed) {
                return mode;
            }
        }
    }
    read_reasons_mode_from_file(root).unwrap_or(ReasonsMode::Warn)
}

/// Like `get_reasons_mode`, but an agent can only make the mode stricter with `TRAIL_REASONS`, never looser.
pub fn get_reasons_mode_for(root: &Path, actor: &Actor) -> ReasonsMode {
    let requested = get_reasons_mode(root);
    if actor.kind != "agent" {
        return requested;
    }
    let floor = read_reasons_mode_from_file(root).unwrap_or(ReasonsMode::Warn);
    let rank = |m: ReasonsMode| match m {
        ReasonsMode::Off => 0,
        ReasonsMode::Warn => 1,
        ReasonsMode::Enforce => 2,
    };
    if rank(requested) >= rank(floor) {
        requested
    } else {
        floor
    }
}

#[cfg(test)]
#[path = "test_reasons.rs"]
mod tests;
