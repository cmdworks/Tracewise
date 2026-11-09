use crate::hook::helpers::{is_trail_protected, recently_edited_by_person, record_refusal, PROTECTED_REASON};
use crate::hook::{HookContext, HookInput, HookOutput};
use crate::util::normalize_and_validate_path;
use std::collections::HashMap;
use tracewise_core::model::Window;

pub fn handle(ctx: &HookContext, input: HookInput) -> Result<HookOutput, String> {
    if ctx.profile.policy && ctx.actor.kind == "agent" && input.paths.iter().any(|p| is_trail_protected(p, &ctx.repo_paths.work_tree)) {
        return Ok(HookOutput { decision: "deny", reason: Some(PROTECTED_REASON.to_string()), notice: None });
    }
    let had_input_paths = !input.paths.is_empty();
    let mut rel_paths: Vec<String> = input.paths.iter().filter_map(|p| normalize_and_validate_path(p, &ctx.repo_paths.work_tree)).collect();
    rel_paths.sort();
    rel_paths.dedup();

    if had_input_paths && rel_paths.is_empty() {
        return Ok(HookOutput { decision: "allow", reason: None, notice: None });
    }

    let mut reminder_notice = None;
    if ctx.actor.kind == "agent" {
        if ctx.profile.coordination {
            if let Some(reason) = recently_edited_by_person(ctx.db, &rel_paths) {
                record_refusal(ctx, &format!("edit of {}", rel_paths.join(", ")), &reason);
                return Ok(HookOutput { decision: "deny", reason: Some(reason), notice: None });
            }
        }
        let mut intent_ids = HashMap::new();
        for p in &rel_paths {
            let covering = ctx.db.find_covering_intent(ctx.agent, Some(ctx.session), p).ok().flatten();
            if covering.is_none() {
                let msg = format!("no intent declared for {p}; run `trail intent add --why <reason> {p}`");
                if ctx.profile.policy {
                    return Ok(HookOutput { decision: "deny", reason: Some(msg), notice: None });
                } else if reminder_notice.is_none() {
                    reminder_notice = Some(msg);
                }
            }
            intent_ids.insert(p.clone(), covering.map(|i| i.id));
        }

        if ctx.profile.coordination {
            match ctx.db.acquire_locks_atomic(&rel_paths, ctx.actor, &intent_ids, ctx.ttl) {
                Ok(Some(conflict)) => {
                    let holder_name = conflict.holder.name.as_deref().unwrap_or(&conflict.holder.kind);
                    let reason = format!("file {} is locked by {holder_name}", conflict.path);
                    record_refusal(ctx, &format!("edit of {}", rel_paths.join(", ")), &reason);
                    return Ok(HookOutput { decision: "deny", reason: Some(reason), notice: None });
                }
                Err(e) => {
                    eprintln!("lock error: {e}");
                }
                Ok(None) => {}
            }
        }
    }

    if ctx.actor.kind == "agent" {
        super::awareness::note_overlap(ctx, &rel_paths);
        if let Some(w) = warn(ctx, &rel_paths) {
            reminder_notice = Some(match reminder_notice {
                Some(r) => format!("{w}\n{r}"),
                None => w,
            });
        }
    }

    let window = Window {
        id: ulid::Ulid::new().to_string(),
        kind: "agent_edit".to_string(),
        actor: ctx.actor.clone(),
        paths: rel_paths,
        cwd: input.cwd,
        start: ctx.now.to_string(),
        end: None,
        command: None,
    };
    let _ = ctx.db.open_window(&window);

    Ok(HookOutput { decision: "allow", reason: None, notice: reminder_notice })
}

/// Marks a `notice` that is the pre-edit warning; an adapter that can show it to the model looks for this prefix.
pub const WARN_PREFIX: &str = "Trail warning: ";

/// `[hooks] warn` (or `TRAIL_WARN`): `off`, `shadow` (log only) or `on` (log and tell the agent). Default `shadow` in the
/// `coordination` profile and `off` in `core`.
pub(crate) fn warn_mode(ctx: &HookContext) -> String {
    let set =
        std::env::var("TRAIL_WARN").ok().or_else(|| tracewise_core::reasons::config_value(&ctx.repo_paths.work_tree, "hooks", "warn"));
    match set.as_deref().map(str::trim) {
        Some(m @ ("off" | "shadow" | "on")) => m.to_string(),
        _ => if ctx.profile.coordination { "shadow" } else { "off" }.to_string(),
    }
}

/// The pre-edit warning: when another actor changed a file after this session's last prompt, one line goes to
/// `shadow-warnings.jsonl` in the data folder (a sample to review for precision). With `warn = "on"` the same finding is also
/// returned as a notice (advisory, the decision stays `allow`). A change is reported once per session and path. No database
/// write; a failure here never changes the hook answer.
fn warn(ctx: &HookContext, rel_paths: &[String]) -> Option<String> {
    let mode = warn_mode(ctx);
    if mode == "off" {
        return None;
    }
    let since = ctx.db.last_prompt_at(ctx.session).ok().flatten()?;
    let file = ctx.repo_paths.data_dir().join("shadow-warnings.jsonl");
    // a file that grew past 1 MB starts again: it is a sample for review, not a log
    if std::fs::metadata(&file).is_ok_and(|m| m.len() > 1_000_000) {
        let _ = std::fs::remove_file(&file);
    }
    let seen = std::fs::read_to_string(&file).unwrap_or_default();
    let mut lines = String::new();
    let mut texts = Vec::new();
    for p in rel_paths {
        let others: Vec<_> = ctx
            .db
            .changes_by_others_since(p, &since, ctx.session, 3)
            .unwrap_or_default()
            .into_iter()
            .filter(|c| !seen.lines().any(|l| l.contains(&format!("\"session\":\"{}\"", ctx.session)) && l.contains(&c.id)))
            .collect();
        let Some(last) = others.first() else { continue };
        let by: Vec<_> = others
            .iter()
            .map(|c| serde_json::json!({"change": c.id, "actor": c.actor.name.as_deref().unwrap_or(&c.actor.kind), "confidence": c.confidence}))
            .collect();
        lines.push_str(
            &serde_json::json!({"kind": "edit", "time": ctx.now, "session": ctx.session, "agent": ctx.agent, "path": p, "since": since, "by": by})
                .to_string(),
        );
        lines.push('\n');
        // names only with a confidence that supports them
        let who = who(&last.confidence, last.actor.name.as_deref());
        texts.push(format!("{p} was changed by {who} at {} after your last prompt [{}]", last.time.start, last.id));
    }
    if lines.is_empty() {
        return None;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&file) {
        let _ = std::io::Write::write_all(&mut f, lines.as_bytes());
    }
    if mode != "on" {
        return None;
    }
    let more = texts.len().saturating_sub(3);
    texts.truncate(3);
    let mut out = format!("{WARN_PREFIX}{}. Re-read before editing; see `trail file <path>`.", texts.join("; "));
    if more > 0 {
        out.push_str(&format!(" And {more} more."));
    }
    Some(out)
}

/// How the warning names the other actor: only a confidence that supports a name gets one.
fn who(confidence: &str, name: Option<&str>) -> String {
    match (confidence, name) {
        ("exact" | "likely", Some(n)) => format!("{n} ({confidence})"),
        _ => "another actor (unnamed)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::who;

    #[test]
    fn a_name_needs_a_confidence_that_supports_it() {
        assert_eq!(who("exact", Some("codex")), "codex (exact)");
        assert_eq!(who("likely", Some("codex")), "codex (likely)");
        for confidence in ["unknown", "candidate", ""] {
            assert_eq!(who(confidence, Some("codex")), "another actor (unnamed)", "{confidence}");
        }
        assert_eq!(who("exact", None), "another actor (unnamed)");
    }
}
