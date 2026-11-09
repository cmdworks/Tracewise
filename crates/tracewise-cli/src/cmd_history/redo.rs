//! Redo command implementation.
use super::*;
use std::path::Path;

pub(crate) struct RedoContext<'a> {
    pub repo_paths: &'a RepoPaths,
    pub blobs: &'a BlobStore,
    pub dry_run: bool,
}

pub(crate) fn run_redo(json: bool, repo_paths: &RepoPaths, id: String, dry_run: bool) {
    let db_path = repo_paths.data_dir().join("trail.db");
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("trail: redo failed: {e}");
            process::exit(1);
        }
    };
    let blobs = BlobStore::new(&repo_paths.data_dir());
    redo_by_id(&db, &blobs, repo_paths, json, &id, dry_run);
}

fn redo_by_id(db: &Database, blobs: &BlobStore, repo_paths: &RepoPaths, json: bool, undo_id: &str, dry_run: bool) {
    let undo_change = match db.get_change(undo_id) {
        Ok(Some(c)) => c,
        _ => {
            eprintln!("trail: change not found: {undo_id}");
            process::exit(1);
        }
    };

    let env_actor = tracewise_core::model::Actor::from_env();
    let target_name = undo_change.actor.name.as_deref().unwrap_or("unknown");
    if !can_actor_undo(env_actor.as_ref(), target_name) {
        eprintln!("trail: refused: agents cannot undo another actor's changes");
        process::exit(1);
    }

    let mut redone_paths = Vec::new();
    let mut skipped = Vec::new();

    let ctx = RedoContext { repo_paths, blobs, dry_run };

    redo_single_change(&undo_change, &ctx, &mut redone_paths, &mut skipped);

    finish_redo(repo_paths, json, redone_paths, skipped, dry_run);
}

pub(crate) fn redo_single_change(change: &Change, ctx: &RedoContext, redone_paths: &mut Vec<String>, skipped: &mut Vec<String>) {
    let path = &change.path;
    let change_slice = [change];
    if let Some(reason) = check_path_binary_or_hash_only(path, ctx.repo_paths, &change_slice, None, ctx.blobs) {
        eprintln!("trail: skipped {path}: cannot redo: {reason}");
        skipped.push(path.to_string());
        return;
    }

    let is_undo = change.op == "undo";
    let (base_hash, theirs_hash) =
        if is_undo { (change.after.as_deref(), change.before.as_deref()) } else { (change.before.as_deref(), change.after.as_deref()) };

    let Some(full_path) = util::safe_join(&ctx.repo_paths.work_tree, path) else {
        skipped.push(path.to_string());
        return;
    };

    if base_hash.is_some() && theirs_hash.is_none() {
        if full_path.exists() {
            if !ctx.dry_run {
                let _ = fs::remove_file(&full_path);
            }
            redone_paths.push(path.to_string());
        }
    } else if base_hash.is_none() && theirs_hash.is_some() {
        redo_create(path, &full_path, theirs_hash.unwrap(), ctx, redone_paths, skipped);
    } else if change.op == "rename" || change.old_path.is_some() {
        redo_rename(change, is_undo, ctx, redone_paths, skipped);
    } else if let (Some(b_hash), Some(t_hash)) = (base_hash, theirs_hash) {
        redo_text_merge(path, &full_path, b_hash, t_hash, ctx, redone_paths, skipped);
    }
}

fn redo_create(
    path: &str,
    full_path: &Path,
    blob_hash: &str,
    ctx: &RedoContext,
    redone_paths: &mut Vec<String>,
    skipped: &mut Vec<String>,
) {
    if full_path.exists() {
        return;
    }
    if let Ok(bytes) = ctx.blobs.read(blob_hash) {
        if !ctx.dry_run {
            if let Some(parent) = full_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let tmp = full_path.with_extension(format!("{}.tmp", ulid::Ulid::new()));
            if fs::write(&tmp, &bytes).is_ok() && fs::rename(&tmp, full_path).is_ok() {
                redone_paths.push(path.to_string());
            } else {
                let _ = fs::remove_file(&tmp);
                skipped.push(path.to_string());
            }
        } else {
            redone_paths.push(path.to_string());
        }
    } else {
        skipped.push(path.to_string());
    }
}

fn redo_rename(change: &Change, is_undo: bool, ctx: &RedoContext, redone_paths: &mut Vec<String>, skipped: &mut Vec<String>) {
    let (src_rel, dst_rel) = if is_undo {
        (change.path.as_str(), change.old_path.as_deref())
    } else {
        (change.old_path.as_deref().unwrap_or(change.path.as_str()), Some(change.path.as_str()))
    };
    let Some(dst_rel) = dst_rel else {
        skipped.push(change.path.clone());
        return;
    };
    let Some(src_full) = util::safe_join(&ctx.repo_paths.work_tree, src_rel) else {
        skipped.push(change.path.clone());
        return;
    };
    let Some(dst_full) = util::safe_join(&ctx.repo_paths.work_tree, dst_rel) else {
        skipped.push(change.path.clone());
        return;
    };

    if src_full.exists() && !dst_full.exists() {
        if !ctx.dry_run {
            if let Some(parent) = dst_full.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::rename(&src_full, &dst_full).is_ok() {
                redone_paths.push(src_rel.to_string());
                redone_paths.push(dst_rel.to_string());
            } else {
                skipped.push(change.path.clone());
            }
        } else {
            redone_paths.push(src_rel.to_string());
            redone_paths.push(dst_rel.to_string());
        }
    } else {
        skipped.push(change.path.clone());
    }
}

fn redo_text_merge(
    path: &str,
    full_path: &Path,
    base_hash: &str,
    theirs_hash: &str,
    ctx: &RedoContext,
    redone_paths: &mut Vec<String>,
    skipped: &mut Vec<String>,
) {
    if !full_path.exists() {
        skipped.push(path.to_string());
        return;
    }

    let current_bytes = match fs::read(full_path) {
        Ok(b) => b,
        Err(_) => {
            skipped.push(path.to_string());
            return;
        }
    };
    let current_text = match String::from_utf8(current_bytes) {
        Ok(s) => s,
        Err(_) => {
            skipped.push(path.to_string());
            return;
        }
    };

    let base_text = match ctx.blobs.read(base_hash) {
        Ok(b) => String::from_utf8(b).unwrap_or_default(),
        Err(_) => {
            skipped.push(path.to_string());
            return;
        }
    };
    let theirs_text = match ctx.blobs.read(theirs_hash) {
        Ok(b) => String::from_utf8(b).unwrap_or_default(),
        Err(_) => {
            skipped.push(path.to_string());
            return;
        }
    };

    let merge_res = tracewise_core::diff::three_way_merge(&base_text, &current_text, &theirs_text);
    if !merge_res.conflicts.is_empty() {
        for conflict in merge_res.conflicts {
            let msg = format!("{path}:{} … conflict with later edits", conflict.line_in_ours);
            skipped.push(msg);
        }
    }

    if merge_res.merged_text != current_text {
        if !ctx.dry_run {
            let tmp = full_path.with_extension(format!("{}.tmp", ulid::Ulid::new()));
            if fs::write(&tmp, merge_res.merged_text.as_bytes()).is_ok() && fs::rename(&tmp, full_path).is_ok() {
                redone_paths.push(path.to_string());
            } else {
                let _ = fs::remove_file(&tmp);
                skipped.push(path.to_string());
            }
        } else {
            redone_paths.push(path.to_string());
        }
    }
}

fn finish_redo(repo_paths: &RepoPaths, json: bool, redone_paths: Vec<String>, skipped: Vec<String>, dry_run: bool) {
    if redone_paths.is_empty() && skipped.is_empty() {
        println!("nothing to do");
        return;
    }

    if !dry_run && !redone_paths.is_empty() {
        let (branch, head) = get_git_info(repo_paths);
        let p_bufs: Vec<PathBuf> = redone_paths.iter().map(PathBuf::from).collect();
        let scan_opts = ScanOptions {
            source: "redo".to_string(),
            actor: Some(get_human_actor()),
            confidence: Some("exact".to_string()),
            op: Some("redo".to_string()),
            ..Default::default()
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
        println!("{}", serde_json::json!({ "redone": redone_paths, "skipped": skipped_paths }));
    } else {
        if !redone_paths.is_empty() {
            println!("redone: {}", redone_paths.join(", "));
        }
        if !skipped.is_empty() {
            println!("skipped: {}", skipped.join(", "));
        }
    }
}
