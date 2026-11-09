use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeRange {
    pub start: String,
    pub end: String,
}

impl TimeRange {
    pub fn now() -> Self {
        let ts = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        Self { start: ts.clone(), end: ts }
    }

    pub fn from_times(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            start: start.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            end: end.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Actor {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

impl Actor {
    /// True when both are the same actor. An agent whose session is unknown matches any session of that agent. A person is one actor: the window opened by the shell hook (named)
    /// and the one a shim opens (no name) are the same human.
    pub fn is_same(&self, other: &Actor) -> bool {
        (self.kind == "human" && other.kind == "human")
            // an agent seen through its hook (with a session) and through the process chain (without) is one agent
            || (self.kind == "agent"
                && other.kind == "agent"
                && self.name == other.name
                && (self.session.is_none() || other.session.is_none()))
            || self == other
    }

    pub fn baseline() -> Self {
        Self { kind: "baseline".to_string(), name: None, session: None }
    }

    pub fn unknown() -> Self {
        Self { kind: "unknown".to_string(), name: None, session: None }
    }

    pub fn agent(name: &str, session: &str) -> Self {
        Self { kind: "agent".to_string(), name: Some(name.to_string()), session: Some(session.to_string()) }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        let parts: Vec<&str> = trimmed.split(':').collect();
        match parts.as_slice() {
            [kind, name, session] if !kind.is_empty() => {
                Some(Self { kind: kind.to_string(), name: Some(name.to_string()), session: Some(session.to_string()) })
            }
            [kind, name] if !kind.is_empty() => Some(Self { kind: kind.to_string(), name: Some(name.to_string()), session: None }),
            [kind] if !kind.is_empty() => Some(Self { kind: kind.to_string(), name: None, session: None }),
            _ => None,
        }
    }

    pub fn from_env() -> Option<Self> {
        let marker = Self::claude_from_env().or_else(Self::antigravity_from_env);
        std::env::var("TRAIL_ACTOR")
            .ok()
            .and_then(|val| Self::parse(&val))
            // An agent cannot pose as a person through TRAIL_ACTOR.
            .filter(|a| marker.is_none() || a.kind != "human")
            .or(marker)
    }

    /// Antigravity runs commands with `ANTIGRAVITY_AGENT=1` and its conversation id, the id its hooks report.
    fn antigravity_from_env() -> Option<Self> {
        if std::env::var("ANTIGRAVITY_AGENT").ok().as_deref() != Some("1") {
            return None;
        }
        let id = std::env::var("ANTIGRAVITY_CONVERSATION_ID").ok().filter(|s| !s.is_empty())?;
        Some(Self::agent("antigravity", &id))
    }

    /// Claude Code runs its Bash tool with `CLAUDECODE=1` and its session id, the same id its hooks report.
    fn claude_from_env() -> Option<Self> {
        if std::env::var("CLAUDECODE").ok().as_deref() != Some("1") {
            return None;
        }
        let id = std::env::var("CLAUDE_CODE_SESSION_ID").ok().filter(|s| !s.is_empty())?;
        Some(Self::agent("claude", &id))
    }
}

impl std::fmt::Display for Actor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.name, &self.session) {
            (Some(name), Some(session)) => write!(f, "{}:{}:{}", self.kind, name, session),
            (Some(name), None) => write!(f, "{}:{}", self.kind, name),
            _ => write!(f, "{}", self.kind),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitInfo {
    pub branch: String,
    pub head: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Change {
    pub id: String,
    pub time: TimeRange,
    pub path: String,
    pub op: String,
    pub old_path: Option<String>,
    pub hunks: Vec<Hunk>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub actor: Actor,
    pub candidates: Vec<Actor>,
    pub confidence: String,
    pub source: String,
    pub command: Option<String>,
    pub intent: Option<String>,
    pub lock_conflict: Option<serde_json::Value>,
    pub device: String,
    pub git: GitInfo,
    pub prev_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undoes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingItem {
    pub id: String,
    pub path: String,
    pub actor: Actor,
    pub time: TimeRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Window {
    pub id: String,
    pub kind: String,
    pub actor: Actor,
    pub paths: Vec<String>,
    pub cwd: Option<String>,
    pub start: String,
    pub end: Option<String>,
    pub command: Option<String>,
}

pub const MAX_COMMAND_TEXT_BYTES: usize = 1024;
pub const TRUNCATION_MARKER: &str = " ... [truncated]";

const MASK: &str = "***";

/// Drops terminal escape sequences and control characters (keeps newline and tab): a reason or command is shown
/// in a terminal, and must not be able to move the cursor, set the title or ring the bell (V06).
pub fn plain_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => match chars.next() {
                Some('[') => {
                    for n in chars.by_ref() {
                        if ('@'..='~').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(n) = chars.next() {
                        if n == '\x07' || (n == '\x1b' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Replaces `prefix` plus a run of letters, digits and `_` (at least `min_tail` long) with the mask.
/// `gh` is special: GitHub token prefixes `ghp_ gho_ ghu_ ghs_ ghr_` need a tail of 20 or more; `github_pat_` the same.
fn mask_prefixed(text: &str, prefix: &str, min_tail: usize) -> String {
    let prefixes: &[&str] = if prefix == "gh" { &["github_pat_", "ghp_", "gho_", "ghu_", "ghs_", "ghr_"] } else { &[] };
    let (mut out, mut i) = (String::new(), 0);
    while i < text.len() {
        let rest = &text[i..];
        let hit = if prefix == "gh" {
            prefixes.iter().find(|p| rest.starts_with(**p)).map(|p| (p.len(), 20))
        } else {
            rest.starts_with(prefix).then_some((prefix.len(), min_tail))
        };
        let tail = hit.map(|(n, _)| rest[n..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').count());
        match (hit, tail) {
            (Some((n, min)), Some(t)) if t >= min => {
                out.push_str(MASK);
                i += n + t;
            }
            _ => {
                let c = rest.chars().next().unwrap();
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    out
}

/// Hides well-known secret shapes before text is stored (V11, V12): AWS access key ids, GitHub tokens,
/// `Bearer` tokens, `--user/-u name:password` and `user:password@` in URLs.
// shortcut: known shapes only, so a password in an unknown format is stored as typed; add a shape when one is found.
pub fn mask_secrets(text: &str) -> String {
    let text = &mask_prefixed(&mask_prefixed(&plain_text(text), "AKIA", 16), "gh", 0);
    let mut out = Vec::new();
    let mut after = None; // the previous word asks for a secret next
    for word in text.split(' ') {
        let mut w = word.to_string();
        if after == Some("bearer") {
            w = MASK.to_string() + if w.ends_with('\'') { "'" } else { "" };
        } else if after == Some("user") {
            if let Some((name, _)) = w.trim_matches('\'').split_once(':') {
                w = format!("{name}:{MASK}");
            }
        } else if let Some(at) = w.find("://").and_then(|i| w[i + 3..].find('@').map(|j| (i + 3, i + 3 + j))) {
            if let Some((name, _)) = w[at.0..at.1].split_once(':') {
                w = format!("{}{name}:{MASK}{}", &w[..at.0], &w[at.1..]);
            }
        }
        after = match w.trim_matches('\'') {
            "--user" | "-u" => Some("user"),
            "Bearer" | "bearer" => Some("bearer"),
            _ => None,
        };
        out.push(w);
    }
    out.join(" ")
}

/// Caps command text at 1 KB (1024 bytes) plus a truncation marker.
pub fn cap_command_text(text: &str) -> String {
    let text = &mask_secrets(text);
    if text.len() <= MAX_COMMAND_TEXT_BYTES {
        text.to_string()
    } else {
        let mut boundary = MAX_COMMAND_TEXT_BYTES;
        while boundary > 0 && !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        format!("{}{}", &text[..boundary], TRUNCATION_MARKER)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRecord {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub cwd: Option<String>,
    pub actor: Actor,
    pub start: String,
    pub end: String,
    pub exit_code: i32,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Intent {
    pub id: String,
    pub actor: Actor,
    pub paths: Vec<String>,
    pub why: String,
    pub task: Option<String>,
    pub created: String,
    pub expires: String,
    pub kind: String,
    #[serde(skip_serializing, default)]
    pub ended: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockRecord {
    pub path: String,
    pub holder: Actor,
    pub since: String,
    pub expires: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

/// Validates that a why string satisfies the quality floor.
pub fn validate_why(why: &str) -> Result<(), &'static str> {
    let trimmed = why.trim();
    if trimmed.len() < 12 {
        return Err("reason too short or generic");
    }
    let clean: String = trimmed.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let generic = ["fix", "update", "wip", "changes", "test", "edit", "misc", "stuff", "todo"];
    if generic.contains(&clean.as_str()) {
        return Err("reason too short or generic");
    }
    Ok(())
}

/// Matches a simple glob pattern against a path.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }
    let p_chars: Vec<char> = pattern.chars().collect();
    let s_chars: Vec<char> = path.chars().collect();
    glob_match_inner(&p_chars, &s_chars)
}

fn glob_match_inner(pat: &[char], s: &[char]) -> bool {
    match (pat, s) {
        ([], []) => true,
        (['*'], _) => !s.contains(&'/'),
        (['*', p_rest @ ..], _) => glob_match_inner(p_rest, s) || (!s.is_empty() && s[0] != '/' && glob_match_inner(pat, &s[1..])),
        ([p, p_rest @ ..], [c, s_rest @ ..]) if *p == *c || *p == '?' => glob_match_inner(p_rest, s_rest),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub path: String,
    pub blob: String,
    pub size: u64,
    pub mtime_ns: i64,
}

#[cfg(test)]
#[path = "test_model.rs"]
mod tests;
