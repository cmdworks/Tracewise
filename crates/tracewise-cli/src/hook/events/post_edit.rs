use crate::hook::helpers::spool_post_edit;
use crate::hook::{HookContext, HookInput, HookOutput};
use crate::util::normalize_and_validate_path;
use std::path::{Path, PathBuf};
use tracewise_core::{scan_repository, scan_specific_paths, ScanOptions};
use tracewise_git::{get_git_info, walk_baseline_files};

pub fn handle(ctx: &HookContext, input: HookInput) -> Result<HookOutput, String> {
    let had_input_paths = !input.paths.is_empty();
    let rel_paths: Vec<String> = input.paths.iter().filter_map(|p| normalize_and_validate_path(p, &ctx.repo_paths.work_tree)).collect();

    if had_input_paths && rel_paths.is_empty() {
        return Ok(HookOutput { decision: "allow", reason: None, notice: None });
    }

    let closed_window = ctx.db.close_latest_window(ctx.agent, ctx.session, "agent_edit", ctx.now).ok().flatten();
    let window_start = closed_window.as_ref().map(|w| w.start.as_str()).unwrap_or(ctx.now);

    let (branch, head) = get_git_info(ctx.repo_paths);
    let data_dir = ctx.repo_paths.data_dir();

    if !rel_paths.is_empty() {
        post_edit_specific(ctx, &data_dir, &input.paths, &rel_paths, &branch, &head, window_start);
    } else {
        post_edit_all(ctx, &data_dir, &branch, &head, window_start);
    }

    Ok(HookOutput { decision: "allow", reason: None, notice: None })
}

fn post_edit_specific(
    ctx: &HookContext,
    data_dir: &Path,
    input_paths: &[String],
    rel_paths: &[String],
    branch: &str,
    head: &str,
    window_start: &str,
) {
    let path_bufs: Vec<PathBuf> = rel_paths.iter().map(PathBuf::from).collect();
    let scan_opts = ScanOptions {
        source: "hook".to_string(),
        actor: Some(ctx.actor.clone()),
        confidence: Some("exact".to_string()),
        predicted_paths: None,
        window_start: Some(window_start.to_string()),
        ..Default::default()
    };
    let change_ids = match scan_specific_paths(&ctx.repo_paths.work_tree, data_dir, branch, head, &scan_opts, &path_bufs) {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("trail post-edit scan_specific_paths error: {e}, spooling");
            spool_post_edit(&ctx.repo_paths.work_tree, data_dir, ctx.actor, input_paths, ctx.repo_paths);
            Vec::new()
        }
    };

    for path_str in rel_paths {
        if let Ok(Some(intent)) = ctx.db.find_covering_intent(ctx.agent, Some(ctx.session), path_str) {
            for cid in &change_ids {
                let _ = ctx.db.link_intent_to_change(cid, &intent.id);
            }
        }
        if let Ok(unknowns) = ctx.db.find_unknown_changes_for_path_since(path_str, window_start) {
            for u in unknowns {
                let _ = ctx.db.upgrade_change_actor(&u.id, ctx.actor, "exact", "hook");
                if let Ok(Some(intent)) = ctx.db.find_covering_intent(ctx.agent, Some(ctx.session), path_str) {
                    let _ = ctx.db.link_intent_to_change(&u.id, &intent.id);
                }
            }
        }
    }
}

fn post_edit_all(ctx: &HookContext, data_dir: &Path, branch: &str, head: &str, window_start: &str) {
    let files = walk_baseline_files(&ctx.repo_paths.work_tree);
    let scan_opts = ScanOptions {
        source: "hook".to_string(),
        actor: Some(ctx.actor.clone()),
        confidence: Some("exact".to_string()),
        predicted_paths: None,
        window_start: Some(window_start.to_string()),
        ..Default::default()
    };
    let change_ids = match scan_repository(&ctx.repo_paths.work_tree, data_dir, branch, head, &scan_opts, &files) {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("trail post-edit scan_repository error: {e}, spooling");
            let file_strs: Vec<String> = files.iter().map(|p| p.to_string_lossy().to_string()).collect();
            spool_post_edit(&ctx.repo_paths.work_tree, data_dir, ctx.actor, &file_strs, ctx.repo_paths);
            Vec::new()
        }
    };
    for cid in &change_ids {
        if let Ok(Some(ch)) = ctx.db.get_change(cid) {
            if let Ok(Some(intent)) = ctx.db.find_covering_intent(ctx.agent, Some(ctx.session), &ch.path) {
                let _ = ctx.db.link_intent_to_change(cid, &intent.id);
            }
        }
    }
    let _ = ctx.db.settle_ambiguous_for(ctx.actor, window_start);
    if let Ok(unknowns) = ctx.db.find_unknown_changes_since(window_start) {
        for u in unknowns {
            let _ = ctx.db.upgrade_change_actor(&u.id, ctx.actor, "exact", "hook");
            if let Ok(Some(intent)) = ctx.db.find_covering_intent(ctx.agent, Some(ctx.session), &u.path) {
                let _ = ctx.db.link_intent_to_change(&u.id, &intent.id);
            }
        }
    }

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
}
