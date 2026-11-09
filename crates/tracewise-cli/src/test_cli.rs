use super::*;

#[test]
fn test_cli_parse() {
    let cli = Cli::parse_from(["tracewise", "status"]);
    assert!(!cli.json);
}

#[test]
fn test_cli_shims_parse() {
    let cli = Cli::parse_from(["tracewise", "shims", "install", "--dir", "/tmp/shims", "--json"]);
    assert!(cli.json);
    match cli.command {
        Some(Commands::Shims { action: ShimsAction::Install { dir } }) => {
            assert_eq!(dir, PathBuf::from("/tmp/shims"));
        }
        _ => panic!("expected Commands::Shims::Install"),
    }

    let cli2 = Cli::parse_from(["tracewise", "shims", "uninstall", "--dir", "/tmp/shims"]);
    assert!(!cli2.json);
    match cli2.command {
        Some(Commands::Shims { action: ShimsAction::Uninstall { dir } }) => {
            assert_eq!(dir, PathBuf::from("/tmp/shims"));
        }
        _ => panic!("expected Commands::Shims::Uninstall"),
    }
}

#[test]
fn test_cli_pending_parse() {
    let cli = Cli::parse_from(["tracewise", "pending", "--json"]);
    assert!(cli.json);
    match cli.command {
        Some(Commands::Pending) => {}
        _ => panic!("expected Commands::Pending"),
    }
}

#[test]
fn test_cli_note_last_parse() {
    let cli = Cli::parse_from(["tracewise", "note", "--last", "document release procedure", "--json"]);
    assert!(cli.json);
    match cli.command {
        Some(Commands::Note { last, change_id, why }) => {
            assert_eq!(last.as_deref(), Some("document release procedure"));
            assert!(change_id.is_none());
            assert!(why.is_none());
        }
        _ => panic!("expected Commands::Note"),
    }
}

#[test]
fn test_cli_run_parse() {
    let cli = Cli::parse_from(["tracewise", "run", "--why", "implement user login flow", "--", "echo", "hello", "world"]);
    match cli.command {
        Some(Commands::Run { why, command }) => {
            assert_eq!(why, "implement user login flow");
            assert_eq!(command, vec!["echo", "hello", "world"]);
        }
        _ => panic!("expected Commands::Run"),
    }
}

#[test]
fn test_cli_proc_chain_parse() {
    let cli = Cli::parse_from(["tracewise", "proc", "chain", "--json"]);
    assert!(cli.json);
    match cli.command {
        Some(Commands::Proc { action: ProcAction::Chain { pid } }) => {
            assert_eq!(pid, None);
        }
        _ => panic!("expected Commands::Proc::Chain"),
    }
}

#[test]
fn test_cli_proc_who_parse() {
    let cli = Cli::parse_from(["tracewise", "proc", "who", "--json"]);
    assert!(cli.json);
    match cli.command {
        Some(Commands::Proc { action: ProcAction::Who { pid } }) => {
            assert_eq!(pid, None);
        }
        _ => panic!("expected Commands::Proc::Who"),
    }
}

#[test]
fn test_cli_agent_parse() {
    let cli = Cli::parse_from(["tracewise", "agent", "install", "claude", "--json"]);
    assert!(cli.json);
    match cli.command {
        Some(Commands::Agent { action: AgentAction::Install { name, scope } }) => {
            assert_eq!(name, "claude");
            assert_eq!(scope, "project");
        }
        _ => panic!("expected Commands::Agent::Install"),
    }
}

#[test]
fn test_cli_hook_from_parse() {
    let cli = Cli::parse_from(["tracewise", "hook", "pre-edit", "--from", "claude", "--record", "/tmp/rec"]);
    match cli.command {
        Some(Commands::Hook { event, from, record }) => {
            assert_eq!(event, "pre-edit");
            assert_eq!(from.as_deref(), Some("claude"));
            assert_eq!(record, Some(PathBuf::from("/tmp/rec")));
        }
        _ => panic!("expected Commands::Hook"),
    }
}

#[test]
fn test_cli_shell_begin_end_parse() {
    let cli_begin = Cli::parse_from(["tracewise", "shell", "begin", "--pid", "12345", "--", "cargo", "test"]);
    match cli_begin.command {
        Some(Commands::Shell { action: ShellAction::Begin { pid, command } }) => {
            assert_eq!(pid, 12345);
            assert_eq!(command, vec!["cargo", "test"]);
        }
        _ => panic!("expected ShellAction::Begin"),
    }

    let cli_end = Cli::parse_from(["tracewise", "shell", "end", "--pid", "12345", "--exit", "0"]);
    match cli_end.command {
        Some(Commands::Shell { action: ShellAction::End { pid, exit } }) => {
            assert_eq!(pid, 12345);
            assert_eq!(exit, 0);
        }
        _ => panic!("expected ShellAction::End"),
    }
}

#[test]
fn test_cli_undo_redo_parse() {
    let cli_undo = Cli::parse_from(["tracewise", "undo", "--actor", "codex", "--since", "1h", "--dry-run"]);
    match cli_undo.command {
        Some(Commands::Undo { id, actor, since, dry_run, strict, .. }) => {
            assert_eq!(id, None);
            assert_eq!(actor.as_deref(), Some("codex"));
            assert_eq!(since.as_deref(), Some("1h"));
            assert!(dry_run);
            assert!(!strict);
        }
        _ => panic!("expected Commands::Undo"),
    }

    let cli_undo_id = Cli::parse_from(["tracewise", "undo", "01M5T3BNN6CDA1X5BEX9QGDD4R", "--strict"]);
    match cli_undo_id.command {
        Some(Commands::Undo { id, actor, since, dry_run, strict, .. }) => {
            assert_eq!(id.as_deref(), Some("01M5T3BNN6CDA1X5BEX9QGDD4R"));
            assert_eq!(actor, None);
            assert_eq!(since, None);
            assert!(!dry_run);
            assert!(strict);
        }
        _ => panic!("expected Commands::Undo"),
    }

    let cli_redo = Cli::parse_from(["tracewise", "redo", "01M5T3BNN6CDA1X5BEX9QGDD4R", "--dry-run"]);
    match cli_redo.command {
        Some(Commands::Redo { id, dry_run }) => {
            assert_eq!(id, "01M5T3BNN6CDA1X5BEX9QGDD4R");
            assert!(dry_run);
        }
        _ => panic!("expected Commands::Redo"),
    }
}
