use crate::hook::{HookContext, HookOutput};

pub fn handle(ctx: &HookContext) -> Result<HookOutput, String> {
    let _ = ctx.db.close_session_windows(ctx.agent, ctx.session, ctx.now);
    let _ = ctx.db.end_session_intents(ctx.agent, ctx.session);
    let _ = ctx.db.release_session_locks_with_flags(ctx.session, &ctx.repo_paths.work_tree);

    Ok(HookOutput { decision: "allow", reason: None, notice: None })
}
