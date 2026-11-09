//! Reverting state: undo and restore commands.
use super::*;

pub(crate) fn can_actor_undo(current_actor: Option<&Actor>, target_actor_name: &str) -> bool {
    if let Some(agent) = current_actor.filter(|a| a.kind == "agent") {
        if agent.name.as_deref() != Some(target_actor_name) {
            return false;
        }
    }
    true
}

pub(crate) fn run_undo(
    json: bool,
    repo_paths: &RepoPaths,
    id: Option<String>,
    mut target: Target,
    since: Option<String>,
    dry_run: bool,
    strict: bool,
) {
    let env_actor = tracewise_core::model::Actor::from_env();
    if let Some(ref name) = target.actor {
        if !can_actor_undo(env_actor.as_ref(), name) {
            eprintln!("tracewise: refused: agents cannot undo another actor's changes");
            process::exit(1);
        }
    }
    // an agent can undo a whole intent or session only for the changes it made itself
    target.restrict_to = env_actor.as_ref().filter(|a| a.kind == "agent").and_then(|a| a.name.clone());

    let db_path = repo_paths.data_dir().join("tracewise.db");
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("tracewise: undo failed: {e}");
            process::exit(1);
        }
    };
    let blobs = BlobStore::new(&repo_paths.data_dir());

    if target.is_set() {
        undo_by_target(&db, &blobs, repo_paths, json, &target, since, dry_run);
    } else if let Some(change_id) = id {
        undo_by_id(&db, &blobs, repo_paths, json, &change_id, dry_run, strict);
    } else {
        eprintln!("tracewise undo: requires a change id, --actor, --intent, --session or --command");
        process::exit(1);
    }
}

/// Which changes a bulk undo selects: all the given fields must match (`--actor`, `--intent`, `--session`). `restrict_to` is
/// set for an agent caller, which may only undo its own changes.
pub(crate) struct Target {
    actor: Option<String>,
    intent: Option<String>,
    session: Option<String>,
    command: Option<String>,
    restrict_to: Option<String>,
}

impl Target {
    pub(crate) fn new(actor: Option<String>, intent: Option<String>, session: Option<String>) -> Self {
        Self { actor, intent, session, command: None, restrict_to: None }
    }

    pub(crate) fn with_command(mut self, command: Option<String>) -> Self {
        self.command = command;
        self
    }

    #[cfg(test)]
    pub(crate) fn actor(name: &str) -> Self {
        Self::new(Some(name.to_string()), None, None)
    }

    fn is_set(&self) -> bool {
        self.actor.is_some() || self.intent.is_some() || self.session.is_some() || self.command.is_some()
    }

    pub(crate) fn matches(&self, c: &Change) -> bool {
        self.actor.as_deref().is_none_or(|a| c.actor.name.as_deref() == Some(a) || c.actor.kind == a)
            && self.intent.as_deref().is_none_or(|i| c.intent.as_deref() == Some(i))
            && self.session.as_deref().is_none_or(|s| c.actor.session.as_deref() == Some(s))
            && self.command.as_deref().is_none_or(|k| c.command.as_deref() == Some(k))
            && self.restrict_to.as_deref().is_none_or(|r| c.actor.name.as_deref() == Some(r))
    }
}

struct RevertContext<'a> {
    window_changes: &'a [Change],
    target: &'a Target,
    repo_paths: &'a RepoPaths,
    blobs: &'a BlobStore,
    dry_run: bool,
}

fn undo_by_target(
    db: &Database,
    blobs: &BlobStore,
    repo_paths: &RepoPaths,
    json: bool,
    target: &Target,
    since: Option<String>,
    dry_run: bool,
) {
    // by actor alone the default window is 24 hours; an intent or a session names its changes exactly, so it has none
    let whole_history = target.intent.is_some() || target.session.is_some() || target.command.is_some();
    let cutoff = match since.as_deref().and_then(parse_since) {
        Some(c) => c,
        None if whole_history => chrono::DateTime::UNIX_EPOCH,
        None => tracewise_core::clock::now() - chrono::Duration::hours(24),
    };
    let cutoff_str = cutoff.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let all_changes = db.list_changes(None, None).unwrap_or_default();
    let window_changes: Vec<Change> = all_changes.into_iter().filter(|c| c.time.start >= cutoff_str).collect();

    let mut actor_paths = Vec::new();
    for c in &window_changes {
        if target.matches(c) && !actor_paths.contains(&c.path) {
            actor_paths.push(c.path.clone());
        }
    }
    actor_paths.sort();

    let mut reverted = Vec::new();
    let mut skipped = Vec::new();
    let mut path_undoes = std::collections::HashMap::new();

    let ctx = RevertContext { window_changes: &window_changes, target, repo_paths, blobs, dry_run };

    for path in actor_paths {
        revert_actor_path(&path, &ctx, &mut reverted, &mut skipped, &mut path_undoes);
    }

    if !dry_run && !reverted.is_empty() {
        let (branch, head) = get_git_info(repo_paths);
        let p_bufs: Vec<PathBuf> = reverted.iter().map(PathBuf::from).collect();
        let scan_opts = ScanOptions {
            source: "undo".to_string(),
            actor: Some(get_human_actor()),
            confidence: Some("exact".to_string()),
            predicted_paths: None,
            window_start: None,
            op: Some("undo".to_string()),
            undoes: None,
            path_undoes: Some(path_undoes),
        };
        let _ = scan_specific_paths(&repo_paths.work_tree, &repo_paths.data_dir(), &branch, &head, &scan_opts, &p_bufs);
    }

    if json {
        let mut skipped_paths = Vec::new();
        for s in &skipped {
            let p = s.split(':').next().unwrap_or(s).trim().to_string();
            if !skipped_paths.contains(&p) {
                skipped_paths.push(p);
            }
        }
        println!("{}", serde_json::json!({ "reverted": reverted, "skipped": skipped_paths }));
    } else {
        println!("{}: {}", if dry_run { "would revert" } else { "reverted" }, reverted.join(", "));
        if !skipped.is_empty() {
            println!("skipped: {}", skipped.join(", "));
        }
    }
}

pub(crate) fn check_path_binary_or_hash_only(
    path: &str,
    repo_paths: &RepoPaths,
    path_changes: &[&Change],
    target: Option<&Target>,
    blobs: &BlobStore,
) -> Option<String> {
    if let Some(full) = util::safe_join(&repo_paths.work_tree, path) {
        if let Ok(bytes) = fs::read(&full) {
            if bytes.contains(&0) {
                return Some("binary file".to_string());
            }
            if bytes.len() as u64 > blobs.max_file() {
                return Some("file was recorded by hash only (too large/binary)".to_string());
            }
        }
    }

    for c in path_changes {
        let matches = target.is_none_or(|t| t.matches(c));
        if matches {
            if let Some(ref h) = c.before {
                match blobs.read(h) {
                    Ok(b) if b.contains(&0) => {
                        return Some("binary file".to_string());
                    }
                    Ok(b) if b.len() as u64 > blobs.max_file() => {
                        return Some("file was recorded by hash only (too large/binary)".to_string());
                    }
                    Err(_) => {
                        return Some("file was recorded by hash only (too large/binary)".to_string());
                    }
                    _ => {}
                }
            }
            if let Some(ref h) = c.after {
                match blobs.read(h) {
                    Ok(b) if b.contains(&0) => {
                        return Some("binary file".to_string());
                    }
                    Ok(b) if b.len() as u64 > blobs.max_file() => {
                        return Some("file was recorded by hash only (too large/binary)".to_string());
                    }
                    Err(_) => {
                        return Some("file was recorded by hash only (too large/binary)".to_string());
                    }
                    _ => {}
                }
            }
        }
    }

    None
}

fn revert_actor_path(
    path: &str,
    ctx: &RevertContext,
    reverted: &mut Vec<String>,
    skipped: &mut Vec<String>,
    path_undoes: &mut std::collections::HashMap<String, Vec<String>>,
) {
    let mut path_changes: Vec<&Change> = ctx.window_changes.iter().filter(|c| c.path == path).collect();
    path_changes.sort_by(|a, b| a.time.start.cmp(&b.time.start));

    if let Some(reason) = check_path_binary_or_hash_only(path, ctx.repo_paths, &path_changes, Some(ctx.target), ctx.blobs) {
        eprintln!("tracewise: skipped {path}: cannot undo: {reason}");
        skipped.push(path.to_string());
        return;
    }

    let Some(full_path) = util::safe_join(&ctx.repo_paths.work_tree, path) else {
        skipped.push(path.to_string());
        return;
    };

    if !full_path.exists() {
        skipped.push(path.to_string());
        return;
    }

    let target_indices: Vec<usize> = path_changes.iter().enumerate().filter(|(_, c)| ctx.target.matches(c)).map(|(i, _)| i).collect();

    if target_indices.is_empty() {
        return;
    }

    let undone_ids: Vec<String> = target_indices.iter().map(|&i| path_changes[i].id.clone()).collect();

    if try_revert_actor_rename(path, path_changes[target_indices[0]], ctx, undone_ids.clone(), reverted, skipped, path_undoes) {
        return;
    }

    let Some(mut current_text) = fs::read(&full_path).ok().and_then(|b| String::from_utf8(b).ok()) else {
        skipped.push(path.to_string());
        return;
    };
    let initial_text = current_text.clone();

    for &idx in target_indices.iter().rev() {
        let c = path_changes[idx];
        let (Some(base_text), Some(theirs_text)) =
            (read_blob_str(ctx.blobs, c.after.as_deref()), read_blob_str(ctx.blobs, c.before.as_deref()))
        else {
            skipped.push(path.to_string());
            return;
        };

        let merge_res = tracewise_core::diff::three_way_merge(&base_text, &current_text, &theirs_text);
        current_text = merge_res.merged_text;

        if !merge_res.conflicts.is_empty() {
            let later_actor = path_changes[idx + 1..]
                .iter()
                .find(|other| !ctx.target.matches(other))
                .map(|other| other.actor.name.as_deref().unwrap_or(other.actor.kind.as_str()))
                .unwrap_or("another actor");

            for conflict in merge_res.conflicts {
                skipped.push(format!("{path}:{} … changed later by {later_actor}", conflict.line_in_ours));
            }
        }
    }

    let first_idx = target_indices[0];
    if first_idx == 0 && path_changes[0].before.is_none() && current_text.trim().is_empty() {
        if !ctx.dry_run {
            let _ = fs::remove_file(&full_path);
        }
        reverted.push(path.to_string());
        path_undoes.insert(path.to_string(), undone_ids);
    } else if current_text != initial_text {
        if !ctx.dry_run {
            let tmp_path = full_path.with_extension(format!("{}.tmp", ulid::Ulid::new()));
            if fs::write(&tmp_path, current_text.as_bytes()).is_ok() {
                if fs::rename(&tmp_path, &full_path).is_ok() {
                    reverted.push(path.to_string());
                    path_undoes.insert(path.to_string(), undone_ids);
                } else {
                    let _ = fs::remove_file(&tmp_path);
                    skipped.push(path.to_string());
                }
            } else {
                skipped.push(path.to_string());
            }
        } else {
            reverted.push(path.to_string());
            path_undoes.insert(path.to_string(), undone_ids);
        }
    }
}

fn read_blob_str(blobs: &BlobStore, h: Option<&str>) -> Option<String> {
    match h {
        Some(hash) => blobs.read(hash).ok().map(|b| String::from_utf8(b).unwrap_or_default()),
        None => Some(String::new()),
    }
}

fn try_revert_actor_rename(
    path: &str,
    first_actor_change: &tracewise_core::model::Change,
    ctx: &RevertContext,
    undone_ids: Vec<String>,
    reverted: &mut Vec<String>,
    skipped: &mut Vec<String>,
    path_undoes: &mut std::collections::HashMap<String, Vec<String>>,
) -> bool {
    if first_actor_change.op != "rename" && first_actor_change.old_path.is_none() {
        return false;
    }
    let Some(ref old_rel) = first_actor_change.old_path else { return false };
    let Some(old_file_path) = util::safe_join(&ctx.repo_paths.work_tree, old_rel) else {
        skipped.push(path.to_string());
        return true;
    };
    if old_file_path.exists() {
        skipped.push(path.to_string());
        return true;
    }
    if !ctx.dry_run {
        if let Some(parent) = old_file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::rename(ctx.repo_paths.work_tree.join(path), &old_file_path).is_err() {
            skipped.push(path.to_string());
            return true;
        }
        if let Some(ref before_blob) = first_actor_change.before {
            if first_actor_change.before != first_actor_change.after {
                if let Ok(bytes) = ctx.blobs.read(before_blob) {
                    let _ = fs::write(&old_file_path, bytes);
                }
            }
        }
    }
    reverted.push(path.to_string());
    reverted.push(old_rel.to_string());
    path_undoes.insert(path.to_string(), undone_ids.clone());
    path_undoes.insert(old_rel.to_string(), undone_ids);
    true
}

fn write_atomic_or_exit(file_path: &Path, bytes: &[u8]) {
    if let Some(parent) = file_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp_path = file_path.with_extension(format!("{}.tmp", ulid::Ulid::new()));
    if let Err(e) = fs::write(&tmp_path, bytes) {
        eprintln!("tracewise: failed to write file: {e}");
        process::exit(1);
    }
    if let Err(e) = fs::rename(&tmp_path, file_path) {
        eprintln!("tracewise: failed to update file: {e}");
        process::exit(1);
    }
}

/// Lists the lines another change touched later (skipped), and stops when that leaves nothing to undo.
fn report_undo_conflicts(path: &str, merge: &tracewise_core::diff::ThreeWayMergeResult, ours: &str) {
    if merge.conflicts.is_empty() {
        return;
    }
    let lines: Vec<String> = merge.conflicts.iter().map(|c| format!("{path}:{}", c.line_in_ours)).collect();
    eprintln!("tracewise: skipped, changed later: {}", lines.join(", "));
    if merge.merged_text == ours {
        eprintln!("tracewise: nothing was undone: the file changed since that change");
        process::exit(1);
    }
}

fn undo_by_id(db: &Database, blobs: &BlobStore, repo_paths: &RepoPaths, json: bool, change_id: &str, dry_run: bool, strict: bool) {
    let Ok(Some(target_change)) = db.get_change(change_id) else {
        eprintln!("tracewise: change not found");
        process::exit(1);
    };
    let env_actor = tracewise_core::model::Actor::from_env();
    if !can_actor_undo(env_actor.as_ref(), target_change.actor.name.as_deref().unwrap_or("unknown")) {
        eprintln!("tracewise: refused: agents cannot undo another actor's changes");
        process::exit(1);
    }
    let Some(file_path) = util::safe_join(&repo_paths.work_tree, &target_change.path) else {
        eprintln!("tracewise: change {} has an unsafe path; refusing", target_change.id);
        process::exit(1);
    };
    let change_slice = [&target_change];
    if let Some(reason) = check_path_binary_or_hash_only(&target_change.path, repo_paths, &change_slice, None, blobs) {
        eprintln!("tracewise: cannot undo: {reason}");
        process::exit(1);
    }
    let current_bytes = fs::read(&file_path).ok();
    let current_hash = current_bytes.as_deref().map(tracewise_core::blob::hash_bytes);
    if strict && current_hash != target_change.after {
        eprintln!("tracewise: changed since");
        process::exit(1);
    }

    let mut paths_to_scan = vec![PathBuf::from(&target_change.path)];

    if target_change.op == "rename" || target_change.old_path.is_some() {
        if let Some(ref old_rel) = target_change.old_path {
            let Some(old_file_path) = util::safe_join(&repo_paths.work_tree, old_rel) else {
                eprintln!("tracewise: unsafe destination path: {old_rel}");
                process::exit(1);
            };
            if old_file_path.exists() {
                eprintln!("tracewise: cannot undo rename: destination {old_rel} already exists");
                process::exit(1);
            }
            if !dry_run {
                if let Some(parent) = old_file_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Err(e) = fs::rename(&file_path, &old_file_path) {
                    eprintln!("tracewise: failed to move file to {old_rel}: {e}");
                    process::exit(1);
                }
                if let Some(ref before_hash) = target_change.before {
                    if target_change.before != target_change.after {
                        if let Ok(bytes) = blobs.read(before_hash) {
                            let _ = fs::write(&old_file_path, bytes);
                        }
                    }
                }
            }
            paths_to_scan.push(PathBuf::from(old_rel));
        }
    } else if current_hash != target_change.after {
        let (Some(base_text), Some(theirs_text)) =
            (read_blob_str(blobs, target_change.after.as_deref()), read_blob_str(blobs, target_change.before.as_deref()))
        else {
            eprintln!("tracewise: cannot undo: file was recorded by hash only (too large/binary)");
            process::exit(1);
        };
        let ours_text = current_bytes.and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
        let merge_res = tracewise_core::diff::three_way_merge(&base_text, &ours_text, &theirs_text);
        report_undo_conflicts(&target_change.path, &merge_res, &ours_text);
        if !dry_run {
            if merge_res.merged_text.trim().is_empty() && target_change.before.is_none() {
                let _ = fs::remove_file(&file_path);
            } else {
                write_atomic_or_exit(&file_path, merge_res.merged_text.as_bytes());
            }
        }
    } else if let Some(before_hash) = target_change.before {
        if !dry_run {
            let Ok(bytes) = blobs.read(&before_hash) else {
                eprintln!("tracewise: cannot undo: file was recorded by hash only (too large/binary)");
                process::exit(1);
            };
            write_atomic_or_exit(&file_path, &bytes);
        }
    } else if !dry_run {
        let _ = fs::remove_file(&file_path);
    }

    if dry_run {
        if json {
            println!("{}", serde_json::json!({ "change": change_id }));
        } else {
            println!("Would undo change {change_id}");
        }
        return;
    }

    let (branch, head) = get_git_info(repo_paths);
    let options = ScanOptions {
        source: "undo".to_string(),
        actor: Some(get_human_actor()),
        confidence: Some("exact".to_string()),
        op: Some("undo".to_string()),
        undoes: Some(vec![change_id.to_string()]),
        ..Default::default()
    };

    let changes =
        scan_specific_paths(&repo_paths.work_tree, &repo_paths.data_dir(), &branch, &head, &options, &paths_to_scan).unwrap_or_default();

    let undo_id = changes.first().cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::json!({ "change": undo_id }));
    } else {
        println!("Undid change {change_id} in {undo_id}");
    }
}

pub(crate) fn run_restore(json: bool, repo_paths: &RepoPaths, path: String, at: String) {
    let (path, file_path) = match tracewise_core::safe_path::resolve_inside(&repo_paths.work_tree, &path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("tracewise: restore refused: {e}");
            process::exit(1);
        }
    };
    let db_path = repo_paths.data_dir().join("tracewise.db");
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("tracewise: restore failed: {e}");
            process::exit(1);
        }
    };
    let blobs = BlobStore::new(&repo_paths.data_dir());

    // as with `undo`: an agent may put back only a file whose newest change is its own, not one someone else changed last
    if let Some(agent) = tracewise_core::model::Actor::from_env().filter(|a| a.kind == "agent") {
        let newest = db.list_changes(Some(&path), Some(1)).unwrap_or_default();
        if newest.first().is_some_and(|c| c.actor.name != agent.name) {
            eprintln!("tracewise: refused: agents cannot restore a file that another actor changed last");
            process::exit(1);
        }
    }

    let target_change = if let Ok(Some(c)) = db.get_change(&at) {
        c
    } else {
        if chrono::DateTime::parse_from_rfc3339(&at).is_err() {
            eprintln!("tracewise: target state not found: '{at}' is neither a change id nor a time (RFC 3339)");
            process::exit(1);
        }
        let changes = db.list_changes(Some(&path), None).unwrap_or_default();
        let found = changes.into_iter().find(|c| c.time.start <= at || c.time.end <= at);
        match found {
            Some(c) => c,
            None => {
                eprintln!("tracewise: target state not found");
                process::exit(1);
            }
        }
    };

    if target_change.path != path {
        eprintln!("tracewise: restore refused: change {} is for '{}', not '{}'", target_change.id, target_change.path, path);
        process::exit(1);
    }

    let blob_hash = match target_change.after {
        Some(h) => h,
        None => {
            eprintln!("tracewise: cannot restore: file was deleted in that change");
            process::exit(1);
        }
    };

    let bytes = match blobs.read(&blob_hash) {
        Ok(b) => b,
        Err(_) => {
            eprintln!("tracewise: cannot restore: file was recorded by hash only (too large/binary)");
            process::exit(1);
        }
    };

    if let Some(parent) = file_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = fs::write(&file_path, bytes) {
        eprintln!("tracewise: failed to write file: {e}");
        process::exit(1);
    }

    let (branch, head) = get_git_info(repo_paths);
    let options = ScanOptions {
        source: "restore".to_string(),
        actor: Some(get_human_actor()),
        confidence: Some("exact".to_string()),
        ..Default::default()
    };

    let changes = scan_specific_paths(&repo_paths.work_tree, &repo_paths.data_dir(), &branch, &head, &options, &[PathBuf::from(&path)])
        .unwrap_or_default();

    let restore_id = changes.first().cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::json!({ "change": restore_id }));
    } else {
        println!("Restored {path} at {at} in {restore_id}");
    }
}
