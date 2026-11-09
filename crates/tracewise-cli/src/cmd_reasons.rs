//! Commands: Intent, Note, Lock, Run, Pending, AdoptCopy, Ps, Background, Doctor.
use super::*;

pub(crate) mod cmd_run;
pub(crate) mod intent;
pub(crate) mod lock;
pub(crate) mod misc;
pub(crate) mod note;

pub(crate) use cmd_run::*;
pub(crate) use intent::*;
pub(crate) use lock::*;
pub(crate) use misc::*;
pub(crate) use note::*;

// Uses tracewise_core::attribute_window(...) via cmd_run::run_cmd_run

pub(crate) fn run(json: bool, repo_paths: &RepoPaths, command: Commands) {
    match command {
        Commands::Intent { action } => run_intent(json, repo_paths, action),
        Commands::Note { last, change_id, why } => run_note(json, repo_paths, last, change_id, why),
        Commands::Lock { action } => run_lock(json, repo_paths, action),
        Commands::Run { why, command } => run_cmd_run(repo_paths, why, command),
        Commands::Pending => run_pending(json, repo_paths),
        Commands::AdoptCopy => run_adopt_copy(json, repo_paths),
        Commands::Ps => run_ps(json, repo_paths),
        Commands::Background { action } => run_background(json, repo_paths, action),
        Commands::Doctor { .. } => run_doctor(json, repo_paths),
        _ => unreachable!("dispatched to the wrong group"),
    }
}
