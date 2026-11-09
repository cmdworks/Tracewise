//! Two more notices built on the same data as the pre-edit warning, both advisory and both behind `[hooks] warn`
//! (`off`, `shadow` = review log only, `on` = also tell the agent; see `pre_edit::warn_mode`):
//!
//! - the first-prompt digest (B7): what others changed since this agent last worked here, shown at a session's first prompt;
//! - the overlap alert (B5): two sessions editing one file within minutes, shown at the next prompt of both.
//!
//! Every finding is one line in `shadow-warnings.jsonl` (with a `kind`) for a later precision review.
use super::pre_edit::warn_mode;
use crate::hook::HookContext;
use std::io::Write;
use std::path::PathBuf;

const DIGEST_PREFIX: &str = "Tracewise digest: ";
const OVERLAP_PREFIX: &str = "Tracewise overlap: ";

/// A notice that starts with a digest or overlap line is shown as it is, without the "since your last prompt" header the
/// ordinary notice gets.
pub fn is_complete(notice: &str) -> bool {
    notice.starts_with(DIGEST_PREFIX) || notice.starts_with(OVERLAP_PREFIX)
}
const OVERLAP_MINUTES: i64 = 5;
const FILE_CAP: u64 = 1_000_000;

/// Only Claude's adapter is known to show these notices to the model; for the other agents `on` behaves as `shadow` until their
/// real replies are captured.
fn delivers(ctx: &HookContext, mode: &str) -> bool {
    mode == "on" && ctx.agent == "claude"
}

fn data_file(ctx: &HookContext, name: &str) -> PathBuf {
    ctx.repo_paths.data_dir().join(name)
}

/// Appends a line to a review/delivery file; a file past 1 MB starts again (a sample, not a log). A failure changes nothing.
fn append(path: &std::path::Path, line: &serde_json::Value) {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > FILE_CAP) {
        let _ = std::fs::remove_file(path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn names(items: &[String], cap: usize) -> String {
    let mut shown: Vec<&str> = items.iter().take(cap).map(String::as_str).collect();
    if items.len() > cap {
        shown.push("...");
    }
    shown.join(", ")
}

/// How the other actors are named: only a confidence that supports a name gets one.
fn who(rows: &[(String, String, String)]) -> String {
    let mut named: Vec<String> =
        rows.iter().filter(|(_, _, c)| matches!(c.as_str(), "exact" | "likely")).map(|(_, w, _)| w.clone()).collect();
    named.sort();
    named.dedup();
    if named.is_empty() {
        "another actor (unnamed)".to_string()
    } else {
        named.join(", ")
    }
}

/// B7. Called at a session's first prompt. `None` unless the mode is `on` and something changed.
pub fn first_prompt_digest(ctx: &HookContext) -> Option<String> {
    let mode = warn_mode(ctx);
    if mode == "off" || ctx.actor.kind != "agent" {
        return None;
    }
    let baseline = ctx.db.last_start_by_actor(ctx.agent).ok().flatten()?;
    let rows = ctx.db.changes_after(&baseline, 200).ok()?;
    if rows.is_empty() {
        return None;
    }
    let mut files: Vec<String> = rows.iter().map(|(p, _, _)| p.clone()).collect();
    files.sort();
    files.dedup();
    let text = format!(
        "{DIGEST_PREFIX}since {} last worked here, {} file{} changed by {}: {}.",
        ctx.agent,
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        who(&rows),
        names(&files, 3)
    );
    append(
        &data_file(ctx, "shadow-warnings.jsonl"),
        &serde_json::json!({"kind": "digest", "time": ctx.now, "session": ctx.session, "agent": ctx.agent, "since": baseline, "files": files.len()}),
    );
    delivers(ctx, &mode).then_some(text)
}

/// B5, at pre-edit. Another agent session has the path in an open edit window, or changed it in the last five minutes: one line
/// goes to the review file and, in `on`, to the delivery file both sessions read at their next prompt (once per pair and path).
pub fn note_overlap(ctx: &HookContext, rel_paths: &[String]) {
    let mode = warn_mode(ctx);
    if mode == "off" || ctx.actor.kind != "agent" {
        return;
    }
    let cutoff =
        (chrono::DateTime::parse_from_rfc3339(ctx.now).map(|t| t.with_timezone(&chrono::Utc)).unwrap_or_else(|_| chrono::Utc::now())
            - chrono::Duration::minutes(OVERLAP_MINUTES))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let open = ctx.db.get_open_windows().unwrap_or_default();
    let delivery = data_file(ctx, "overlaps.jsonl");
    let seen = std::fs::read_to_string(&delivery).unwrap_or_default();
    for p in rel_paths {
        let mut others: Vec<(String, String)> = open // (agent, session)
            .iter()
            .filter(|w| w.kind == "agent_edit" && w.paths.contains(p) && w.actor.session.as_deref() != Some(ctx.session))
            .filter_map(|w| Some((w.actor.name.clone()?, w.actor.session.clone()?)))
            .collect();
        for c in ctx.db.changes_by_others_since(p, &cutoff, ctx.session, 5).unwrap_or_default() {
            // a person's change has no session, so only agent sessions are paired
            if let (Some(n), Some(s)) = (c.actor.name, c.actor.session) {
                others.push((n, s));
            }
        }
        others.sort();
        others.dedup();
        for (name, session) in others {
            let mut pair = [ctx.session.to_string(), session.clone()];
            pair.sort();
            let key = format!("{}|{}|{p}", pair[0], pair[1]);
            if seen.contains(&format!("\"key\":{}", serde_json::json!(key))) {
                continue;
            }
            let line = serde_json::json!({"kind": "overlap", "key": key, "time": ctx.now, "path": p, "sessions": pair,
                "text": format!("{} and {name} are both editing {p}", ctx.agent)});
            append(&data_file(ctx, "shadow-warnings.jsonl"), &line);
            if delivers(ctx, &mode) {
                append(&delivery, &line);
            }
        }
    }
}

/// B5, at a prompt: the overlap lines that name this session and are newer than its previous prompt (or ten minutes old at a
/// first prompt). Empty unless the mode is `on`.
pub fn overlaps_for_prompt(ctx: &HookContext, previous_prompt: Option<&str>) -> Vec<String> {
    if !delivers(ctx, &warn_mode(ctx)) {
        return Vec::new();
    }
    let since = previous_prompt.map(String::from).unwrap_or_else(|| {
        (chrono::Utc::now() - chrono::Duration::minutes(2 * OVERLAP_MINUTES)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    });
    let text = std::fs::read_to_string(data_file(ctx, "overlaps.jsonl")).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["time"].as_str().is_some_and(|t| t > since.as_str()))
        .filter(|v| v["sessions"].as_array().is_some_and(|a| a.iter().any(|s| s == ctx.session)))
        .filter_map(|v| v["text"].as_str().map(|t| format!("{OVERLAP_PREFIX}{t}; coordinate before you both change it")))
        .collect()
}
