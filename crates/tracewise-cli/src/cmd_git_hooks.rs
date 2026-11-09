//! Git hooks for the git clients that never run `~/.trail/bin/git` (GitHub Desktop, Tower, Fork, the git built into editors).
//!
//! `trail git-hooks install` writes five small scripts into the repository's hooks folder; each one calls `trail git-hook <name>`.
//! The wrapper sets `TRAIL_GIT_DEPTH` for the git it starts, and the scripts do nothing when it is set, so a change made from a terminal
//! is recorded once (by the wrapper) and a change made from a git app is recorded once (by the hook). A hook that was already there is
//! kept as `<name>.pre-trail` and still runs after Tracewise's part; `uninstall` puts it back.
use crate::wrapper::{self, GitCommandRecordArgs, GitPreState};
use std::fs;
use std::path::{Path, PathBuf};
use tracewise_core::db::Database;
use tracewise_core::ScanOptions;
use tracewise_git::{get_git_info, RepoPaths};

const HOOKS: [&str; 5] = ["pre-commit", "post-commit", "post-checkout", "post-merge", "post-rewrite"];
const MARK: &str = "# trail-managed git hook";

/// What the hook script for `name` is, with Tracewise at `trail_bin`.
pub(crate) fn script(name: &str, trail_bin: &str) -> String {
    let quoted = trail_bin.replace('\'', r"'\''");
    // only the commit gate may stop git; every other outcome of Tracewise lets git go on (fail open)
    let gate = if name == "pre-commit" { "  [ \"$rc\" -eq 1 ] && exit 1\n" } else { "" };
    format!(
        "#!/bin/sh\n{MARK}: `trail git-hooks uninstall` removes this file and puts back {name}.pre-trail\nT='{quoted}'\n[ -x \"$T\" ] || T=$(command -v trail 2>/dev/null)\nif [ -n \"$T\" ] && [ -z \"$TRAIL_GIT_DEPTH\" ]; then\n  \"$T\" git-hook {name} \"$@\"\n  rc=$?\n{gate}fi\nP=\"$(dirname \"$0\")/{name}.pre-trail\"\n[ -x \"$P\" ] && exec \"$P\" \"$@\"\nexit 0\n"
    )
}

fn hooks_dir(repo: &RepoPaths) -> PathBuf {
    repo.git_common_dir.join("hooks")
}

fn is_ours(path: &Path) -> bool {
    fs::read_to_string(path).is_ok_and(|t| t.contains(MARK))
}

/// `core.hooksPath` when it is set: a tool such as husky owns the hooks there and Tracewise does not edit them.
fn custom_hooks_path(repo: &RepoPaths) -> Option<String> {
    let git = wrapper::find_real_git(None);
    let out = std::process::Command::new(git)
        .arg("-C")
        .arg(&repo.work_tree)
        .args(["config", "--get", "core.hooksPath"])
        .env("TRAIL_GIT_DEPTH", "1")
        .output()
        .ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !v.is_empty()).then_some(v)
}

fn trail_binary() -> String {
    let installed = crate::setup::Env::current().bin().join("trail");
    let exe = if installed.is_file() { installed } else { std::env::current_exe().unwrap_or_else(|_| PathBuf::from("trail")) };
    exe.to_string_lossy().to_string()
}

/// How each hook stands: `trail`, `trail+previous` (a hook that was there still runs), `other` (someone else's, not chained) or `none`.
pub(crate) fn status(repo: &RepoPaths) -> Vec<(&'static str, &'static str)> {
    let dir = hooks_dir(repo);
    HOOKS
        .iter()
        .map(|&name| {
            let p = dir.join(name);
            let s = if is_ours(&p) {
                if dir.join(format!("{name}.pre-trail")).is_file() {
                    "trail+previous"
                } else {
                    "trail"
                }
            } else if p.is_file() {
                "other"
            } else {
                "none"
            };
            (name, s)
        })
        .collect()
}

/// Writes the five hooks. An existing hook of the person's is moved to `<name>.pre-trail` first. Idempotent.
pub(crate) fn install(repo: &RepoPaths) -> Result<(), String> {
    if let Some(p) = custom_hooks_path(repo) {
        return Err(format!("core.hooksPath is set ({p}): Trail does not edit hooks there"));
    }
    let dir = hooks_dir(repo);
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let bin = trail_binary();
    for name in HOOKS {
        let path = dir.join(name);
        let new = script(name, &bin);
        if path.is_file() && !is_ours(&path) {
            let keep = dir.join(format!("{name}.pre-trail"));
            if !keep.exists() {
                fs::rename(&path, &keep).map_err(|e| format!("cannot keep the existing {name} hook: {e}"))?;
            }
        }
        if fs::read_to_string(&path).ok().as_deref() != Some(&new) {
            fs::write(&path, &new).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
        tracewise_platform::private::set_mode(&path, 0o755).map_err(|e| format!("cannot make {} executable: {e}", path.display()))?;
    }
    Ok(())
}

/// Removes Tracewise's hooks and puts back the ones it had moved aside.
pub(crate) fn uninstall(repo: &RepoPaths) {
    let dir = hooks_dir(repo);
    for name in HOOKS {
        let path = dir.join(name);
        if is_ours(&path) {
            let _ = fs::remove_file(&path);
            let keep = dir.join(format!("{name}.pre-trail"));
            if keep.is_file() {
                let _ = fs::rename(&keep, &path);
            }
        }
    }
}

pub(crate) fn run(json: bool, repo: &RepoPaths, action: &crate::cli::GitHooksAction) {
    use crate::cli::GitHooksAction as A;
    let result = match action {
        A::Install => install(repo),
        A::Uninstall => {
            uninstall(repo);
            Ok(())
        }
        A::Status => Ok(()),
    };
    if let Err(e) = &result {
        eprintln!("trail: {e}");
    }
    let rows = status(repo);
    if json {
        let hooks: serde_json::Map<String, serde_json::Value> = rows.iter().map(|(n, s)| (n.to_string(), (*s).into())).collect();
        println!("{}", serde_json::json!({ "hooks": hooks, "ok": result.is_ok() }));
    } else {
        for (n, s) in &rows {
            println!("{n:<14} {s}");
        }
    }
    if result.is_err() {
        std::process::exit(1);
    }
}

fn git_out(git: &Path, work_tree: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(git).arg("-C").arg(work_tree).args(args).env("TRAIL_GIT_DEPTH", "1").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// Called by the hook scripts. Never fails, except `pre-commit` when the commit gate refuses (exit 1).
pub(crate) fn run_hook(repo: &RepoPaths, name: &str, args: &[String]) {
    // the wrapper started this git and records it itself
    if std::env::var("TRAIL_GIT_DEPTH").is_ok() {
        return;
    }
    let db_path = repo.data_dir().join("trail.db");
    if !db_path.is_file() {
        return;
    }
    if name == "pre-commit" {
        wrapper::check_commit_gate(&Some(repo.clone()), &["commit".to_string()]);
        return;
    }
    let Ok(db) = Database::open(&db_path) else { return };
    let git = wrapper::find_real_git(None);
    let (actor, confidence) = wrapper::determine_git_actor(&db);
    let (branch, head_info) = get_git_info(repo);
    let opts = ScanOptions {
        source: "git".to_string(),
        actor: Some(actor.clone()),
        confidence: Some(confidence),
        predicted_paths: None,
        window_start: None,
        ..Default::default()
    };
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let pre = GitPreState { old_head: String::new(), now: now.clone(), dirty_before: Vec::new() };
    let data_dir = repo.data_dir();

    // the two commits a move went between: post-checkout hands them over, merge and rewrite leave ORIG_HEAD
    let range: Option<(String, String)> = match name {
        "post-checkout" if args.get(2).map(String::as_str) == Some("1") => match (args.first(), args.get(1)) {
            (Some(o), Some(n)) if o != n && !o.chars().all(|c| c == '0') => Some((o.clone(), n.clone())),
            _ => None,
        },
        "post-merge" | "post-rewrite" => git_out(&git, &repo.work_tree, &["rev-parse", "-q", "--verify", "ORIG_HEAD"])
            .zip(git_out(&git, &repo.work_tree, &["rev-parse", "-q", "--verify", "HEAD"]))
            .filter(|(o, n)| o != n),
        _ => None,
    };
    let (command, ids) = match name {
        "post-commit" => ("commit", wrapper::scan_diff_tree(&git, repo, &data_dir, &branch, &head_info, &opts, &["--root", "-m", "HEAD"])),
        _ => {
            let cmd = match name {
                "post-checkout" => "checkout",
                "post-merge" => "merge",
                _ => "rebase",
            };
            let ids = match &range {
                Some((o, n)) => wrapper::scan_diff_tree(&git, repo, &data_dir, &branch, &head_info, &opts, &[o, n]),
                None => wrapper::scan_status_changes(&git, repo, cmd, &pre, &[], &opts),
            };
            (cmd, ids)
        }
    };
    let git_args = vec![command.to_string(), format!("(git app, {name} hook)")];
    wrapper::record_mutating_git_command(GitCommandRecordArgs {
        db: &db,
        real_git: &git,
        repo_paths: repo,
        git_args: &git_args,
        current_dir: &repo.work_tree,
        actor,
        start_time: &now,
        exit_code: 0,
        change_ids: &ids,
    });
}

#[cfg(test)]
#[path = "test_cmd_git_hooks.rs"]
mod tests;
