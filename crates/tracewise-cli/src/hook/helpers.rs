use crate::util::normalize_path;
use std::path::{Component, Path, PathBuf};
use tracewise_core::db::Database;
use tracewise_core::model::Actor;
use tracewise_git::{get_git_info, RepoPaths};

pub const PROTECTED_REASON: &str = "Tracewise's data and the agents' hook settings can only be changed by a person; ask the user to do it";

pub const AGY_EDIT_TOOLS: [&str; 8] = [
    "write_file",
    "write_to_file",
    "replace_file_content",
    "multi_replace_file_content",
    "edit_file",
    "create_file",
    "delete_file",
    "apply_patch",
];

/// Files an agent must never write: Tracewise's data, its home, and the agents' hook configs (the agent could remove its own hooks).
pub fn is_tracewise_protected(raw: &str, work_tree: &Path) -> bool {
    let p = Path::new(raw);
    let joined = if p.is_absolute() { p.to_path_buf() } else { work_tree.join(p) };
    let mut abs = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::ParentDir => {
                abs.pop();
            }
            Component::CurDir => {}
            other => abs.push(other),
        }
    }
    if abs.starts_with(tracewise_core::get_tracewise_home()) {
        return true;
    }
    let Ok(rel) = abs.strip_prefix(work_tree) else {
        return false;
    };
    let rel = rel.to_string_lossy();
    rel == ".git/tracewise"
        || rel == ".tracewiseconfig"
        || rel.starts_with(".tracewise/adapters")
        || rel.starts_with(".git/tracewise/")
        || rel.starts_with(".claude/settings")
        || rel.starts_with(".agents/hooks")
        || rel.starts_with(".codex/")
        || rel == ".gemini/settings.json"
        || rel == ".cursor/hooks.json"
}

/// Antigravity sends `{conversationId, toolCall:{name,args}, workspacePaths}` and accepts only
/// `{"decision","reason"}` back. It has no session-end event: `Stop` with `fullyIdle` is the end.
/// How long after a person's (or an unhooked editor's) save an agent must leave the file alone.
pub fn edit_grace_secs() -> i64 {
    std::env::var("TRACEWISE_EDIT_GRACE_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(20)
}

/// A deny reason when somebody else saved one of `paths` a moment ago: they are probably still editing it.
pub fn recently_edited_by_person(db: &Database, paths: &[String]) -> Option<String> {
    let grace = edit_grace_secs();
    if grace <= 0 {
        return None;
    }
    paths.iter().find_map(|p| {
        let c = db.recent_person_edit(p, grace).ok().flatten()?;
        let ago = chrono::DateTime::parse_from_rfc3339(&c.time.start)
            .map(|t| (tracewise_core::clock::now() - t.with_timezone(&chrono::Utc)).num_seconds())
            .unwrap_or(0)
            .max(0);
        Some(format!(
            "{p} was saved {ago}s ago by someone editing it (a person or an editor); wait about {}s and try again, or ask them",
            (grace - ago).max(1)
        ))
    })
}

pub fn spool_post_edit(work_tree: &Path, data_dir: &Path, actor: &Actor, paths: &[String], repo_paths: &RepoPaths) {
    let spool_dir = data_dir.join("spool");
    let _ = std::fs::create_dir_all(&spool_dir);

    let blob_store = tracewise_core::blob::BlobStore::new(data_dir);
    let (branch, head) = get_git_info(repo_paths);
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    for p in paths {
        let rel_p = normalize_path(p, work_tree);
        let full_p = work_tree.join(&rel_p);
        if !full_p.is_file() {
            continue;
        }
        let content = match std::fs::read(&full_p) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let after_hash = match blob_store.store(&content) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let before_hash = get_git_file_blob(work_tree, &rel_p);
        let hunks = if let Some(ref b_hash) = before_hash {
            if let Ok(b_bytes) = blob_store.read(b_hash) {
                tracewise_core::diff::compute_hunks(Some(&b_bytes), Some(&content))
            } else {
                Vec::new()
            }
        } else {
            tracewise_core::diff::compute_hunks(None, Some(&content))
        };

        let ch = tracewise_core::model::Change {
            id: ulid::Ulid::new().to_string(),
            time: tracewise_core::model::TimeRange { start: now.clone(), end: now.clone() },
            path: rel_p,
            op: if before_hash.is_some() { "modify".to_string() } else { "create".to_string() },
            old_path: None,
            hunks,
            before: before_hash,
            after: Some(after_hash),
            actor: actor.clone(),
            candidates: vec![actor.clone()],
            confidence: "exact".to_string(),
            source: "hook".to_string(),
            command: None,
            intent: None,
            lock_conflict: None,
            device: String::new(),
            git: tracewise_core::model::GitInfo { branch: branch.clone(), head: head.clone() },
            prev_hash: None,
            undoes: None,
        };

        let spool_file = spool_dir.join(format!("{}.json", ch.id));
        if let Ok(json_str) = serde_json::to_string(&ch) {
            let _ = std::fs::write(&spool_file, json_str);
        }
    }
}

pub fn get_git_file_blob(work_tree: &Path, rel_path: &str) -> Option<String> {
    let out = std::process::Command::new("/usr/bin/git")
        .arg("-C")
        .arg(work_tree)
        .arg("rev-parse")
        .arg(format!("HEAD:{rel_path}"))
        .env("TRACEWISE_GIT_DEPTH", "1")
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.len() == 40 {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
#[path = "test_helpers.rs"]
pub mod protected_tests;

/// Leaves a `refused` command record for an edit or command that coordination denied (a lock, a person's recent save), so the
/// refusal can be seen in the history and not only in the agent's own reply. Fails open: the answer to the agent is unchanged.
pub fn record_refusal(ctx: &crate::hook::HookContext, what: &str, reason: &str) {
    let rec = tracewise_core::model::CommandRecord {
        id: ulid::Ulid::new().to_string(),
        kind: "refused".to_string(),
        text: format!("{what} refused: {reason}"),
        cwd: None,
        actor: ctx.actor.clone(),
        start: ctx.now.to_string(),
        end: ctx.now.to_string(),
        exit_code: 1,
        changes: Vec::new(),
    };
    let _ = ctx.db.insert_command(&rec);
}
