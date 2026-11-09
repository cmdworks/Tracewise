use crate::hook::{HookContext, HookOutput};

pub fn handle(ctx: &HookContext) -> Result<HookOutput, String> {
    // The turn is over: no tool call of this session is still running. Windows a failed call left open
    // would make every other actor's change ambiguous.
    let _ = ctx.db.close_session_windows(ctx.agent, ctx.session, ctx.now);
    if ctx.actor.kind == "agent" {
        if ctx.profile.policy {
            if let Ok(changes) = ctx.db.get_agent_changes_for_session(ctx.agent, ctx.session) {
                let mut uncovered = Vec::new();
                for c in changes {
                    if c.intent.is_none() {
                        if let Ok(Some(intent)) = ctx.db.find_any_covering_intent(ctx.agent, Some(ctx.session), &c.path) {
                            let _ = ctx.db.link_intent_to_change(&c.id, &intent.id);
                        } else if !uncovered.contains(&c.path) {
                            uncovered.push(c.path);
                        }
                    }
                }
                if !uncovered.is_empty() {
                    return Ok(HookOutput {
                        decision: "block",
                        reason: Some(format!("session has changes with no reason: {}", uncovered.join(", "))),
                        notice: None,
                    });
                }
            }
        }
        // The turn is done: free the files for the next agent if coordination was on.
        if ctx.profile.coordination {
            let _ = ctx.db.release_session_locks_with_flags(ctx.session, &ctx.repo_paths.work_tree);
        }
    }

    Ok(HookOutput { decision: "allow", reason: None, notice: None })
}
