//! Commands: Daemon, Shell, CommandHistory, GitWrapper.
use super::*;

pub(crate) mod commands;
pub(crate) mod daemon;
pub(crate) mod shell;
#[cfg(test)]
mod tests;

pub(crate) use commands::*;
pub(crate) use daemon::*;
pub(crate) use shell::*;

// Uses tracewise_core::attribute_window(...) via shell::run_shell_action

pub(crate) fn run(json: bool, repo_paths: &RepoPaths, command: Commands) {
    match command {
        Commands::Daemon { action } => match action {
            DaemonAction::ScanPaths => daemon_scan_paths(repo_paths),
            DaemonAction::Active => daemon_active(repo_paths),
            DaemonAction::Sweep => daemon_sweep(repo_paths),
            DaemonAction::Run
            | DaemonAction::Start
            | DaemonAction::Stop
            | DaemonAction::Status
            | DaemonAction::Install { .. }
            | DaemonAction::Uninstall => unreachable!("handled without a repository"),
        },
        Commands::Shell { action } => run_shell_action(json, repo_paths, action),
        Commands::CommandHistory => run_command_history(json, repo_paths),
        Commands::GitWrapper { action: GitWrapperAction::Install { dir } } => run_git_wrapper_install(json, dir),
        _ => unreachable!("dispatched to the wrong group"),
    }
}
