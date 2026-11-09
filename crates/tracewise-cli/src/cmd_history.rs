//! Commands: Init, Status, Log, Show, Scan, Blame, Why, Diff, Undo, Restore, Gc, Verify.
use super::*;

pub(crate) mod ignore;
pub(crate) mod inspect;
pub(crate) mod prune;
pub(crate) mod redo;
pub(crate) mod repo;
pub(crate) mod report;
pub(crate) mod revert;
#[cfg(test)]
mod tests;

pub(crate) use inspect::*;
pub(crate) use redo::*;
pub(crate) use repo::*;
pub(crate) use revert::*;

pub(crate) fn run(json: bool, repo_paths: &RepoPaths, command: Commands) {
    match command {
        Commands::Init => run_init(json, repo_paths),
        Commands::Status => run_status(json, repo_paths),
        Commands::Scan => run_scan(json, repo_paths),
        Commands::Gc => run_gc(json, repo_paths),
        Commands::Verify => run_verify(json, repo_paths),
        Commands::Ignore { patterns } => ignore::run_ignore(json, repo_paths, patterns),
        Commands::Prune { paths, older_than, dry_run, yes, backup } => {
            prune::run_prune(json, repo_paths, prune::PruneArgs { paths, older_than, dry_run, yes, backup })
        }
        Commands::Log { path, limit, actor, since, grep, after, group } => {
            util::require_valid_since(since.as_deref());
            run_log(json, repo_paths, path, limit, inspect::LogFilter { actor, since, grep, after, group })
        }
        Commands::Show { id } => run_show(json, repo_paths, id),
        Commands::File { path, limit } => run_file(json, repo_paths, path, limit),
        Commands::Report { actor, intent, session, since } => {
            util::require_valid_since(since.as_deref());
            report::run_report(json, repo_paths, revert::Target::new(actor, intent, session), since)
        }
        Commands::Blame { path, at } => run_blame(json, repo_paths, path, at),
        Commands::Why { target } => run_why(json, repo_paths, target),
        Commands::Diff { since, path } => {
            util::require_valid_since(since.as_deref());
            run_diff(json, repo_paths, since, path)
        }
        Commands::Undo { id, actor, intent, session, command, since, dry_run, strict } => {
            util::require_valid_since(since.as_deref());
            run_undo(json, repo_paths, id, revert::Target::new(actor, intent, session).with_command(command), since, dry_run, strict)
        }
        Commands::Redo { id, dry_run } => run_redo(json, repo_paths, id, dry_run),
        Commands::Restore { path, at } => run_restore(json, repo_paths, path, at),
        _ => unreachable!("dispatched to the wrong group"),
    }
}
