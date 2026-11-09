//! The part of the `eslogger` sensor that can be tested without root: reading Apple's JSON, keeping only what is
//! needed, and the compact line that travels to the user's daemon.
//!
//! `eslogger` is "NOT API in any sense" (its manual page): every field lookup here can fail, and a line that does
//! not have the expected shape is skipped and counted, never fatal.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Light preset: everything that creates, renames, truncates, deletes, clones or flags a file. Measured at about
/// 3 % of a core while an agent works. `write` (520 events/s on a busy Mac) is only in the full preset.
pub const LIGHT_EVENTS: &[&str] =
    &["create", "rename", "unlink", "truncate", "clone", "link", "exchangedata", "setflags", "setextattr", "deleteextattr"];
pub const FULL_EVENTS: &[&str] =
    &["create", "rename", "unlink", "truncate", "clone", "link", "exchangedata", "write", "setflags", "setextattr", "deleteextattr"];

pub fn events_for(preset: &str) -> &'static [&'static str] {
    if preset == "full" {
        FULL_EVENTS
    } else {
        LIGHT_EVENTS
    }
}

/// Who did it, as far as one event says. Process environments are never read, kept or forwarded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Writer {
    #[serde(rename = "x")]
    pub exe: String,
    #[serde(rename = "s", default, skip_serializing_if = "Option::is_none")]
    pub signing_id: Option<String>,
    #[serde(rename = "t", default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(rename = "i")]
    pub pid: i64,
    #[serde(rename = "v", default)]
    pub pidversion: i64,
    #[serde(rename = "u", default, skip_serializing_if = "Option::is_none")]
    pub euid: Option<i64>,
    #[serde(rename = "pp", default, skip_serializing_if = "Option::is_none")]
    pub ppid: Option<i64>,
    #[serde(rename = "op", default, skip_serializing_if = "Option::is_none")]
    pub original_ppid: Option<i64>,
}

/// One file event, reduced to the compact form that is sent to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorEvent {
    #[serde(rename = "k")]
    pub kind: String,
    #[serde(rename = "p")]
    pub paths: Vec<String>,
    #[serde(rename = "w")]
    pub writer: Writer,
    #[serde(rename = "t", default)]
    pub time: String,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // one value per line, short-lived: boxing would only add an allocation
pub enum Parsed {
    /// A file event of a kind we use, with its writer.
    Event(SensorEvent),
    /// A valid line that is not for us (another event kind).
    Ignored,
    /// Not JSON, or without the fields a file event must have.
    Skipped,
}

/// `/` is written as `\/` in eslogger's JSON: a plain path never matches the raw line, so prefixes are searched in
/// both spellings.
pub fn prefix_needles(prefix: &str) -> Vec<String> {
    let p = prefix.trim_end_matches('/');
    vec![p.to_string(), p.replace('/', "\\/")]
}

/// Cheap test on the raw line, before any parsing: does it mention a watched path at all?
pub fn mentions_prefix(line: &str, needles: &[String]) -> bool {
    needles.iter().any(|n| line.contains(n.as_str()))
}

fn collect_paths(v: &Value, acc: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(p)) = map.get("path") {
                acc.push(p.clone());
            }
            if let (Some(Value::Object(dir)), Some(Value::String(name))) = (map.get("dir"), map.get("filename")) {
                if let Some(Value::String(d)) = dir.get("path") {
                    acc.push(format!("{}/{}", d.trim_end_matches('/'), name));
                }
            }
            for key in ["target_dir"] {
                if let Some(Value::Object(dir)) = map.get(key) {
                    if let Some(Value::String(d)) = dir.get("path") {
                        for nk in ["target_name", "target_filename"] {
                            if let Some(Value::String(name)) = map.get(nk) {
                                acc.push(format!("{}/{}", d.trim_end_matches('/'), name));
                            }
                        }
                    }
                }
            }
            // the folders in `dir`, `target_dir` and `parent_dir` are context, not the changed file
            for (key, child) in map {
                if !matches!(key.as_str(), "dir" | "target_dir" | "parent_dir") {
                    collect_paths(child, acc);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect_paths(i, acc)),
        _ => {}
    }
}

fn int(v: &Value) -> Option<i64> {
    v.as_i64()
}

/// Reads one line of `eslogger` output.
pub fn parse_line(line: &str, wanted: &[&str]) -> Parsed {
    let Ok(root) = serde_json::from_str::<Value>(line) else {
        return Parsed::Skipped;
    };
    let Some(event) = root.get("event").and_then(Value::as_object) else {
        return Parsed::Skipped;
    };
    let Some((kind, body)) = event.iter().next() else {
        return Parsed::Skipped;
    };
    if !wanted.contains(&kind.as_str()) {
        return Parsed::Ignored;
    }
    let Some(process) = root.get("process") else {
        return Parsed::Skipped;
    };
    let token = process.get("audit_token");
    let (Some(exe), Some(pid)) =
        (process.get("executable").and_then(|e| e.get("path")).and_then(Value::as_str), token.and_then(|t| t.get("pid")).and_then(int))
    else {
        return Parsed::Skipped;
    };
    let mut paths = Vec::new();
    collect_paths(body, &mut paths);
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Parsed::Skipped;
    }
    let text = |k: &str| process.get(k).and_then(Value::as_str).map(str::to_string);
    Parsed::Event(SensorEvent {
        kind: kind.clone(),
        paths,
        writer: Writer {
            exe: exe.to_string(),
            signing_id: text("signing_id"),
            team_id: text("team_id"),
            pid,
            pidversion: token.and_then(|t| t.get("pidversion")).and_then(int).unwrap_or(0),
            euid: token.and_then(|t| t.get("euid")).and_then(int),
            ppid: process.get("ppid").and_then(int),
            original_ppid: process.get("original_ppid").and_then(int),
        },
        time: root.get("time").and_then(Value::as_str).unwrap_or_default().to_string(),
    })
}

/// What the sensor does to one raw line: prefilter, parse, keep only paths under a watched prefix.
pub fn process_line(line: &str, needles: &[String], prefixes: &[String], wanted: &[&str]) -> Parsed {
    if !mentions_prefix(line, needles) {
        // Cheap path: most of the machine's events. A kind we do not use or a line we cannot read is
        // indistinguishable from "not for us" here, and that is the point of not parsing it.
        return Parsed::Ignored;
    }
    match parse_line(line, wanted) {
        Parsed::Event(mut ev) => {
            ev.paths.retain(|p| prefixes.iter().any(|pre| under(p, pre)));
            if ev.paths.is_empty() {
                Parsed::Ignored
            } else {
                Parsed::Event(ev)
            }
        }
        other => other,
    }
}

/// `path` is `prefix` or inside it (a prefix is a folder, not a string prefix: `/a/b` is not under `/a/bc`).
pub fn under(path: &str, prefix: &str) -> bool {
    let pre = prefix.trim_end_matches('/');
    path == pre || path.strip_prefix(pre).is_some_and(|rest| rest.starts_with('/'))
}

/// Messages from the daemon (one JSON object per line).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum ToSensor {
    /// First message: what to watch and which event preset.
    Hello {
        v: u32,
        prefixes: Vec<String>,
        events: String,
    },
    Start,
    Stop,
    /// The daemon does not accept this peer.
    Reject,
}

/// Messages from the sensor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FromSensor {
    Status { st: Status },
    Event(SensorEvent),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// eslogger is running now
    pub running: bool,
    /// lines read from eslogger / events forwarded / lines skipped as unreadable
    pub seen: u64,
    pub forwarded: u64,
    pub skipped: u64,
    /// why it is not running when it should be: `full-disk-access`, `not-root`, `missing`, `format`, `exited: …`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macos: Option<String>,
}

#[cfg(test)]
#[path = "test_lib.rs"]
mod tests;
