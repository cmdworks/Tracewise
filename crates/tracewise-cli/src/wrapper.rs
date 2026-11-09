use crate::util::get_human_actor;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use tracewise_core::db::Database;
use tracewise_core::model::{Actor, CommandRecord};
use tracewise_core::ScanOptions;
use tracewise_git::{get_git_info, RepoPaths};

pub(crate) mod commit_gate;
pub(crate) mod exec;
pub(crate) mod sync;
#[cfg(test)]
mod tests;

pub(crate) use commit_gate::*;
pub use exec::*;
pub(crate) use sync::*;
pub use tracewise_git::{is_read_only_git, is_worktree_mutating_git};

pub fn run_git_wrapper(git_args: &[String]) {
    handle_git_depth_fallback(git_args);

    if is_read_only_git(git_args) {
        fast_read_only_git_exec(git_args);
    }

    let real_git = find_real_git(None);
    let pre = prepare_git_pre_state(&real_git, git_args);

    std::env::set_var("TRACEWISE_GIT_DEPTH", "1");
    let status = std::process::Command::new(&real_git)
        .args(git_args)
        .env("TRACEWISE_GIT_DEPTH", "1")
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();
    let exit_code = status.map(|s| s.code().unwrap_or(1)).unwrap_or(1);

    let current_dir = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => {
            process::exit(exit_code);
        }
    };

    handle_clone_sync(git_args, exit_code, &current_dir);

    let repo_paths = match RepoPaths::discover(&current_dir) {
        Some(p) => p,
        None => {
            process::exit(exit_code);
        }
    };
    let data_dir = repo_paths.data_dir();
    let db_path = data_dir.join("tracewise.db");
    if !db_path.exists() {
        process::exit(exit_code);
    }

    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(_) => {
            process::exit(exit_code);
        }
    };

    let (actor, confidence) = determine_git_actor(&db);

    let subcmd = git_args.iter().find(|a| !a.starts_with('-')).map(|s| s.as_str()).unwrap_or("");

    let (_branch, head) = get_git_info(&repo_paths);
    let scan_opts = ScanOptions {
        source: "git".to_string(),
        actor: Some(actor.clone()),
        confidence: Some(confidence),
        predicted_paths: None,
        window_start: None,
        ..Default::default()
    };

    let change_ids = scan_git_changes(&real_git, &repo_paths, subcmd, &head, &pre, git_args, &scan_opts);

    if is_worktree_mutating_git(git_args) {
        record_mutating_git_command(GitCommandRecordArgs {
            db: &db,
            real_git: &real_git,
            repo_paths: &repo_paths,
            git_args,
            current_dir: &current_dir,
            actor,
            start_time: &pre.now,
            exit_code,
            change_ids: &change_ids,
        });
    }
    process::exit(exit_code);
}
