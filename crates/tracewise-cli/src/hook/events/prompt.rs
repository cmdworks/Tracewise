use super::awareness::{first_prompt_digest, overlaps_for_prompt};
use crate::hook::{HookContext, HookOutput};

/// Header the ordinary notice gets from an adapter; a notice that `is_complete` already carries its own.
const CHANGES_HEADER: &str = "Tracewise: since your last prompt, other actors changed files you work on:\n";

pub fn handle(ctx: &HookContext) -> Result<HookOutput, String> {
    // a new prompt: the previous turn's tool calls are over
    let _ = ctx.db.close_session_windows(ctx.agent, ctx.session, ctx.now);
    let prev_prompt_time = ctx.db.get_and_update_last_prompt(ctx.session, ctx.now).ok().flatten();
    let changes_notice = prev_prompt_time.as_deref().and_then(|since| changes_since(ctx, since));
    let lead: Vec<String> = match prev_prompt_time {
        None => first_prompt_digest(ctx).into_iter().chain(overlaps_for_prompt(ctx, None)).collect(),
        Some(ref since) => overlaps_for_prompt(ctx, Some(since)),
    };
    let notice = if lead.is_empty() { changes_notice } else { Some(complete(&lead, changes_notice)) };
    Ok(HookOutput { decision: "allow", reason: None, notice })
}

/// A notice that carries a digest or overlap line: those lines first (each starts with its own prefix), then the ordinary
/// changes under their own header, so an adapter can print the whole text as it is.
fn complete(lead: &[String], changes: Option<String>) -> String {
    let mut out = lead.join("\n");
    if let Some(c) = changes {
        out.push('\n');
        out.push_str(CHANGES_HEADER);
        out.push_str(&c);
    }
    out
}

fn changes_since(ctx: &HookContext, since_time: &str) -> Option<String> {
    let paths = ctx.db.get_session_declared_or_held_paths(ctx.session).ok()?;
    let changes = ctx.db.get_changes_since_for_paths(since_time, ctx.session, &paths).ok()?;
    if changes.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    for c in changes.iter().take(20) {
        let actor_label = c.actor.name.as_deref().unwrap_or(&c.actor.kind);
        let why_str = c.intent.as_ref().and_then(|iid| ctx.db.get_intent(iid).ok().flatten()).map(|i| i.why);
        if let Some(why) = why_str {
            lines.push(format!("{} {} {} (why: {}) [{}]", actor_label, c.op, c.path, why, c.id));
        } else {
            lines.push(format!("{} {} {} [{}]", actor_label, c.op, c.path, c.id));
        }
    }
    if changes.len() > 20 {
        lines.push(format!("... and {} more", changes.len() - 20));
    }
    Some(lines.join("\n"))
}
