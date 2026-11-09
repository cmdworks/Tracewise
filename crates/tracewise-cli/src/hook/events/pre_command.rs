use crate::hook::helpers::{is_trail_protected, recently_edited_by_person, PROTECTED_REASON};
use crate::hook::{HookContext, HookInput, HookOutput};
use crate::util::normalize_and_validate_path;
use std::collections::HashMap;
use std::path::PathBuf;
use tracewise_core::model::Window;
use tracewise_core::predict::predict_written_files;

pub fn handle(ctx: &HookContext, input: HookInput) -> Result<HookOutput, String> {
    let cmd = input.command.unwrap_or_default();
    let predicted = predict_written_files(&cmd);
    if ctx.profile.policy && ctx.actor.kind == "agent" {
        if tracewise_core::predict::may_write_anywhere(&cmd) && !ctx.db.has_open_intent(ctx.agent, Some(ctx.session)).unwrap_or(true) {
            return Ok(HookOutput {
                decision: "deny",
                reason: Some(
                    "this command can write any file or keep running in the background; declare a reason first: `trail intent add --why <reason> <files>`"
                        .to_string(),
                ),
                notice: None,
            });
        }
        if predicted.iter().any(|p| is_trail_protected(&p.to_string_lossy(), &ctx.repo_paths.work_tree)) {
            return Ok(HookOutput { decision: "deny", reason: Some(PROTECTED_REASON.to_string()), notice: None });
        }
    }
    let mut rel_predicted: Vec<String> =
        predicted.into_iter().filter_map(|p| normalize_and_validate_path(&p.to_string_lossy(), &ctx.repo_paths.work_tree)).collect();
    rel_predicted.sort();
    rel_predicted.dedup();

    if ctx.actor.kind == "agent" && ctx.profile.coordination {
        if let Some(err_output) = check_agent_command_locks(ctx, &rel_predicted)? {
            return Ok(err_output);
        }
    }

    if ctx.actor.kind == "agent" {
        check_exports(ctx, &cmd, input.cwd.as_deref());
    }

    if ctx.actor.kind == "agent" && tracewise_core::predict::starts_background(&cmd) {
        open_background_window(ctx, &cmd, input.cwd.clone());
    }

    let window = Window {
        id: ulid::Ulid::new().to_string(),
        kind: "agent_command".to_string(),
        actor: ctx.actor.clone(),
        paths: rel_predicted,
        cwd: input.cwd,
        start: ctx.now.to_string(),
        end: None,
        command: Some(cmd),
    };
    let _ = ctx.db.open_window(&window);

    Ok(HookOutput { decision: "allow", reason: None, notice: None })
}

fn check_exports(ctx: &HookContext, cmd: &str, cwd_opt: Option<&str>) {
    let cwd = cwd_opt.map(PathBuf::from).unwrap_or_else(|| ctx.repo_paths.work_tree.clone());
    for words in tracewise_core::predict::command_words(cmd) {
        let prog = words[0].rsplit('/').next().unwrap_or(&words[0]).to_string();
        if let Some(dest) = crate::util::export_destination(&ctx.repo_paths.work_tree, &cwd, &prog, &words[1..]) {
            crate::util::record_export(ctx.db, ctx.actor, &prog, &words[1..], &dest);
        }
    }
}

fn open_background_window(ctx: &HookContext, cmd: &str, cwd: Option<String>) {
    let ttl: i64 = std::env::var("TRAIL_BACKGROUND_TTL").ok().and_then(|v| v.parse().ok()).unwrap_or(3600);
    let expires = (tracewise_core::clock::now() + chrono::Duration::seconds(ttl)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let _ = ctx.db.open_window(&Window {
        id: format!("bg_{}", ulid::Ulid::new()),
        kind: "agent_background".to_string(),
        actor: ctx.actor.clone(),
        paths: vec!["*".to_string()],
        cwd,
        start: ctx.now.to_string(),
        end: Some(expires),
        command: Some(cmd.to_string()),
    });
}

fn check_agent_command_locks(ctx: &HookContext, rel_predicted: &[String]) -> Result<Option<HookOutput>, String> {
    if let Some(reason) = recently_edited_by_person(ctx.db, rel_predicted) {
        return Ok(Some(HookOutput { decision: "deny", reason: Some(reason), notice: None }));
    }
    let mut intent_ids = HashMap::new();
    for p in rel_predicted {
        let covering = ctx.db.find_covering_intent(ctx.agent, Some(ctx.session), p).ok().flatten();
        if covering.is_none() {
            return Ok(Some(HookOutput {
                decision: "deny",
                reason: Some(format!("command predicted to write {p} with no intent declared; run `trail intent add --why <reason> {p}`")),
                notice: None,
            }));
        }
        intent_ids.insert(p.clone(), covering.map(|i| i.id));
    }

    match ctx.db.acquire_locks_atomic(rel_predicted, ctx.actor, &intent_ids, ctx.ttl) {
        Ok(Some(conflict)) => {
            let holder_name = conflict.holder.name.as_deref().unwrap_or(&conflict.holder.kind);
            let reason = format!("file {} is locked by {holder_name}", conflict.path);
            crate::hook::helpers::record_refusal(ctx, &format!("command writing {}", rel_predicted.join(", ")), &reason);
            Ok(Some(HookOutput { decision: "deny", reason: Some(reason), notice: None }))
        }
        Err(e) => {
            eprintln!("lock error: {e}");
            Ok(None)
        }
        Ok(None) => Ok(None),
    }
}
