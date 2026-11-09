use crate::hook::{HookContext, HookOutput};
use std::collections::HashSet;
use tracewise_core::model::Window;
use tracewise_core::{scan_repository, ScanOptions};
use tracewise_git::{get_git_info, walk_baseline_files};

pub fn handle(ctx: &HookContext) -> Result<HookOutput, String> {
    let closed_window = ctx.db.close_latest_window(ctx.agent, ctx.session, "agent_command", ctx.now).ok().flatten();
    let (window_start, predicted_set): (String, HashSet<String>) = if let Some(ref w) = closed_window {
        let set = w.paths.iter().cloned().collect();
        (w.start.clone(), set)
    } else {
        (ctx.now.to_string(), HashSet::new())
    };

    // `ls`, `sleep 40`, `git status`: nothing this command did can have changed a file, so a change that
    // appeared meanwhile (an editor's save) is somebody else's and must not be claimed for the agent.
    if predicted_set.is_empty()
        && closed_window.as_ref().and_then(|w| w.command.as_deref()).is_some_and(tracewise_core::predict::only_reads)
    {
        return Ok(HookOutput { decision: "allow", reason: None, notice: None });
    }

    let (branch, head) = get_git_info(ctx.repo_paths);
    let files = walk_baseline_files(&ctx.repo_paths.work_tree);
    let data_dir = ctx.repo_paths.data_dir();

    let scan_opts = ScanOptions {
        source: "hook".to_string(),
        actor: Some(ctx.actor.clone()),
        confidence: None,
        predicted_paths: Some(predicted_set.clone()),
        window_start: Some(window_start.clone()),
        ..Default::default()
    };
    let change_ids = scan_repository(&ctx.repo_paths.work_tree, &data_dir, &branch, &head, &scan_opts, &files).unwrap_or_default();

    let window_for_attr = closed_window.clone().unwrap_or_else(|| Window {
        id: format!("hook_cmd_{}", ulid::Ulid::new()),
        kind: "agent_command".to_string(),
        actor: ctx.actor.clone(),
        paths: predicted_set.iter().cloned().collect(),
        cwd: Some(ctx.repo_paths.work_tree.to_string_lossy().to_string()),
        start: window_start.clone(),
        end: Some(ctx.now.to_string()),
        command: None,
    });
    let _ = tracewise_core::attribute_window(ctx.db, &window_for_attr, ctx.actor, !predicted_set.is_empty(), false);

    let reasons_mode = crate::util::reasons_mode_for(ctx.db, &ctx.repo_paths.work_tree, ctx.actor);
    if reasons_mode.should_warn(ctx.actor) {
        for cid in &change_ids {
            if let Ok(Some(ch)) = ctx.db.get_change(cid) {
                if ch.intent.is_none() {
                    eprintln!("trail: {} changed ({}) without a reason. Add one: trail note {} \"<why>\"", ch.path, ch.id, ch.id);
                }
            }
        }
    }

    Ok(HookOutput { decision: "allow", reason: None, notice: None })
}
