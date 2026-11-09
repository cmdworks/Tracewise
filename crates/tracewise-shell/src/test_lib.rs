use super::*;

#[test]
fn test_shell_snippets() {
    assert!(zsh_interactive().contains("add-zsh-hook"));
    let zsh_non_int = zsh_non_interactive("/usr/local/bin/tracewise");
    assert!(zsh_non_int.contains("/usr/local/bin/tracewise"));
    assert!(zsh_non_int.contains("TRACEWISE_SHELL_ACTIVE"));
    assert!(zsh_non_int.contains("|| exit 1"));
    assert!(zsh_non_int.contains("TRAPEXIT"));

    let bash_non_int = bash_non_interactive("/opt/tracewise/bin/tracewise");
    assert!(bash_non_int.contains("/opt/tracewise/bin/tracewise"));
    assert!(bash_non_int.contains("TRACEWISE_SHELL_ACTIVE"));
    assert!(bash_non_int.contains("|| exit 1"));
    assert!(bash_non_int.contains("shell end --pid $$ --exit $?' EXIT"));
    // a helper whose name starts with an underscore is dropped by shell snapshots (Claude Code), leaving the trap dangling
    assert!(!zsh_non_int.contains("_tracewise_exit") && !bash_non_int.contains("_tracewise_exit"));
}
