use clap::CommandFactory;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use tracewise_core::blob::BlobStore;
use tracewise_core::db::Database;
use tracewise_core::diff::format_unified_diff;
use tracewise_core::model::{validate_why, Actor, Change, CommandRecord, Intent, Window};
use tracewise_core::predict::predict_written_files;
use tracewise_core::{record_baseline, scan_repository, scan_specific_paths, scan_specific_paths_with_db, ScanOptions};
use tracewise_git::{get_git_info, walk_baseline_files, RepoPaths};
use util::*;

mod bulk;
mod cli;
mod cmd_adapters;
mod cmd_agents;
mod cmd_capture;
mod cmd_config;
mod cmd_git_hooks;
mod cmd_history;
mod cmd_reasons;
mod cmd_repos;
mod cmd_share;
mod cmd_shims;
mod cmd_strict;
mod fast_shell;
mod hook;
mod self_heal;
mod setup;
mod shim_detect;
mod shim_install;
mod unhooked;
mod util;
mod wrapper;

pub use cli::*;
use fast_shell::{fast_shell_begin, fast_shell_end, try_fast_shell};

fn handle_early_commands(cli: &Cli) -> bool {
    if let Some(Commands::Hook { ref event, ref from, ref record }) = cli.command {
        hook::handle_hook(event, from.as_deref(), record.as_deref());
        return true;
    }

    match cli.command {
        Some(Commands::Setup { ref login, no_start, no_shell_rc, strict, ref events, ref profile }) => {
            let login = match login.as_str() {
                "yes" => setup::Login::Yes,
                "no" => setup::Login::No,
                _ => setup::Login::Ask,
            };
            setup::run_setup(cli.json, login, no_start, no_shell_rc, strict, events, profile);
            return true;
        }
        Some(Commands::Uninstall { purge }) => {
            setup::run_uninstall(cli.json, purge);
            return true;
        }
        Some(Commands::Doctor { all: true }) => {
            setup::run_doctor_all(cli.json);
            return true;
        }
        _ => {}
    }

    if let Some(Commands::Daemon { ref action }) = cli.command {
        if matches!(
            action,
            DaemonAction::Run
                | DaemonAction::Start
                | DaemonAction::Stop
                | DaemonAction::Status
                | DaemonAction::Install { .. }
                | DaemonAction::Uninstall
        ) {
            let here = std::env::current_dir().ok().and_then(|d| RepoPaths::discover(&d));
            cmd_capture::daemon_global(cli.json, action.clone(), here.as_ref());
            return true;
        }
    }

    if let Some(Commands::Shell { action: ShellAction::Init { ref shell, non_interactive } }) = cli.command {
        handle_shell_init(shell, non_interactive);
        return true;
    }

    if let Some(Commands::GitWrapper { action: GitWrapperAction::Install { ref dir } }) = cli.command {
        install_git_wrapper(cli.json, dir);
        return true;
    }

    if let Some(Commands::Strict { .. }) = cli.command {
        if let Some(ref command) = cli.command {
            cmd_strict::run(cli.json, command.clone());
        }
        return true;
    }

    if let Some(Commands::Shims { .. }) = cli.command {
        if let Some(ref command) = cli.command {
            cmd_shims::run(cli.json, command.clone());
        }
        return true;
    }

    if let Some(Commands::Config { ref action }) = cli.command {
        let here = std::env::current_dir().ok().and_then(|d| RepoPaths::discover(&d));
        let project = here.as_ref().map(|r| r.work_tree.as_path());
        match action {
            ConfigAction::Get { key } => cmd_config::get(cli.json, key.clone(), project),
            ConfigAction::Set { key, value, global } => cmd_config::set(cli.json, key, value, *global, project),
        }
        return true;
    }

    if let Some(Commands::Repos) = cli.command {
        cmd_repos::run(cli.json);
        return true;
    }

    if let Some(Commands::Proc { ref action }) = cli.command {
        handle_proc_action(cli.json, action);
        return true;
    }

    false
}

fn handle_shell_init(shell: &str, non_interactive: bool) {
    let tracewise_bin = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "tracewise".to_string());

    if non_interactive {
        match shell {
            "zsh" => print!("{}", tracewise_shell::zsh_non_interactive(&tracewise_bin)),
            "bash" => print!("{}", tracewise_shell::bash_non_interactive(&tracewise_bin)),
            _ => {
                eprintln!("tracewise: unsupported shell '{}'", shell);
                process::exit(1);
            }
        }
        return;
    }

    match shell {
        "zsh" => print!("{}", tracewise_shell::zsh_interactive()),
        "fish" => print!("{}", tracewise_shell::fish_interactive()),
        "pwsh" | "powershell" => print!("{}", tracewise_platform::shell::pwsh_interactive()),
        "nu" | "nushell" => print!("{}", tracewise_platform::shell::nu_interactive()),
        _ => {
            eprintln!("tracewise: unsupported shell '{}'", shell);
            process::exit(1);
        }
    }
}

fn install_git_wrapper(json: bool, dir: &Path) {
    let _ = fs::create_dir_all(dir);
    let symlink_path = dir.join("git");
    let _ = fs::remove_file(&symlink_path);

    let self_exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("tracewise: failed to get current executable: {e}");
            process::exit(1);
        }
    };

    let target_bin = self_exe.parent().map(|p| p.join("tracewise-git")).filter(|p| p.is_file()).unwrap_or_else(|| self_exe.clone());

    if let Err(e) = tracewise_platform::shims::create_shim_link(&target_bin, &symlink_path) {
        eprintln!("tracewise: failed to create symlink: {e}");
        process::exit(1);
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "installed": true,
                "path": symlink_path.to_string_lossy(),
            })
        );
    } else {
        println!("Installed git wrapper to {}", symlink_path.display());
    }
}

fn handle_proc_action(json: bool, action: &ProcAction) {
    match action {
        ProcAction::Chain { pid } => {
            let target_pid = pid.unwrap_or_else(tracewise_proc::parent_pid);
            let chain = tracewise_proc::process_chain(target_pid);
            if json {
                println!("{}", serde_json::to_string(&chain).unwrap_or_else(|_| "[]".to_string()));
            } else {
                for p in &chain {
                    println!("{:>7} {:>7} {:<16} {}", p.pid, p.ppid, p.name, p.path);
                }
            }
        }
        ProcAction::Who { pid } => {
            let res = tracewise_proc::who(*pid, None);
            if json {
                println!("{}", serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string()));
            } else {
                let match_info = match &res.matched {
                    Some(m) => format!(" (process: {} {})", m.pid, m.name),
                    None => String::new(),
                };
                println!("{}:{} ({}){}", res.kind, res.name, res.source, match_info);
            }
        }
    }
}

/// Commands that read or change the history: without `tracewise init` they have nothing to work on.
fn needs_history(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Log { .. }
            | Commands::Show { .. }
            | Commands::File { .. }
            | Commands::Report { .. }
            | Commands::Blame { .. }
            | Commands::Why { .. }
            | Commands::Diff { .. }
            | Commands::Undo { .. }
            | Commands::Redo { .. }
            | Commands::Restore { .. }
            | Commands::Verify
            | Commands::Gc
            | Commands::Prune { .. }
            | Commands::Pending
            | Commands::Note { .. }
            | Commands::Lock { .. }
    )
}

fn dispatch_repo_command(json: bool, repo_paths: &RepoPaths, command: Commands) {
    if needs_history(&command) && !repo_paths.data_dir().join("tracewise.db").is_file() {
        eprintln!("tracewise: this repository has no Tracewise history yet; run `tracewise init` first");
        process::exit(1);
    }
    match command {
        Commands::Init
        | Commands::Status
        | Commands::Log { .. }
        | Commands::Show { .. }
        | Commands::File { .. }
        | Commands::Report { .. }
        | Commands::Scan
        | Commands::Blame { .. }
        | Commands::Why { .. }
        | Commands::Diff { .. }
        | Commands::Undo { .. }
        | Commands::Redo { .. }
        | Commands::Restore { .. }
        | Commands::Gc
        | Commands::Prune { .. }
        | Commands::Ignore { .. }
        | Commands::Verify => cmd_history::run(json, repo_paths, command),
        Commands::Daemon { .. } | Commands::Shell { .. } | Commands::CommandHistory | Commands::GitWrapper { .. } => {
            cmd_capture::run(json, repo_paths, command)
        }
        Commands::Doctor { .. }
        | Commands::Intent { .. }
        | Commands::Note { .. }
        | Commands::Lock { .. }
        | Commands::Ps
        | Commands::AdoptCopy
        | Commands::Background { .. }
        | Commands::Run { .. }
        | Commands::Pending => cmd_reasons::run(json, repo_paths, command),
        Commands::Share { .. } | Commands::Push | Commands::Fetch | Commands::Rebuild => cmd_share::run(json, repo_paths, command),
        Commands::Setup { .. } | Commands::Uninstall { .. } | Commands::Shims { .. } | Commands::Repos | Commands::Config { .. } => {
            unreachable!("handled before repository lookup")
        }
        Commands::GitHooks { action } => cmd_git_hooks::run(json, repo_paths, &action),
        Commands::GitHook { name, args } => cmd_git_hooks::run_hook(repo_paths, &name, &args),
        Commands::Agent { action } => match action {
            AgentAction::Install { name, scope } => {
                if let Err(e) = cmd_agents::handle_agent_install(repo_paths, &name, &scope, json) {
                    eprintln!("tracewise: {e}");
                    process::exit(1);
                }
            }
            AgentAction::Uninstall { name } => {
                let _ = cmd_agents::handle_agent_uninstall(repo_paths, &name, json);
            }
            AgentAction::Status => {
                let _ = cmd_agents::handle_agent_status(repo_paths, json);
            }
            AgentAction::List => {
                let _ = cmd_adapters::handle_agent_list(repo_paths, json);
            }
            AgentAction::Check { file, payload } => {
                let _ = cmd_adapters::handle_agent_check(&file, &payload, json);
            }
        },
        Commands::Hook { .. } | Commands::Proc { .. } | Commands::Strict { .. } => {
            unreachable!()
        }
    }
}

/// The whole command line; `tracewise` and its old name `tracewise` both run this.
pub fn main() {
    tracewise_platform::signal::reset_sigpipe();
    if cmd_shims::try_fast_read_only_shim() {
        return;
    }

    let os_args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let shim_tool = os_args.first().and_then(|a| Path::new(a).file_name()?.to_str()).filter(|n| cmd_shims::SHIM_TOOLS.contains(n));
    if let Some(tool) = shim_tool {
        tracewise_shim::exec_if_non_utf8(tool, &os_args[1..], Path::new(&os_args[0]).parent());
    }
    let args: Vec<String> = std::env::args().collect();
    let bin_name = args.first().and_then(|a| Path::new(a).file_name().and_then(|n| n.to_str())).unwrap_or("");
    if bin_name == "git" {
        wrapper::run_git_wrapper(&args[1..]);
        return;
    }

    if cmd_shims::SHIM_TOOLS.contains(&bin_name) {
        cmd_shims::run_shim(bin_name, &args[1..]);
    }

    if try_fast_shell(&args) {
        return;
    }

    let cli = parse_cli();

    if handle_early_commands(&cli) {
        return;
    }

    if let Some(ref dir) = cli.directory {
        if std::env::set_current_dir(dir).is_err() {
            eprintln!("tracewise: not a git repository");
            process::exit(2);
        }
    }

    let current_dir = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => {
            eprintln!("tracewise: not a git repository");
            process::exit(2);
        }
    };

    let repo_paths = match RepoPaths::discover(&current_dir) {
        Some(paths) => paths,
        None => {
            eprintln!("tracewise: not a git repository");
            process::exit(2);
        }
    };

    let db_path = repo_paths.data_dir().join("tracewise.db");
    if db_path.exists() {
        if let Ok(db) = Database::open(&db_path) {
            let _ = db.import_spool(&repo_paths.data_dir());
            let _ = db.clean_expired_locks(&repo_paths.work_tree);
        }
    }

    if !matches!(cli.command, Some(Commands::AdoptCopy)) {
        if let Some(original) = util::copy_of(&repo_paths) {
            eprintln!(
                "tracewise: this folder is a copy of {} (same tracewise id); its history is separate from now on. \
                 Run `tracewise adopt-copy` to give it its own id before pushing.",
                original.display()
            );
        }
    }

    let json = cli.json;
    match cli.command {
        Some(command) => dispatch_repo_command(json, &repo_paths, command),
        None => {
            let _ = Cli::command().print_help();
            println!();
        }
    }
}

#[cfg(test)]
pub static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
#[path = "test_main.rs"]
mod tests;

fn parse_cli() -> Cli {
    use clap::Parser;
    Cli::parse()
}
