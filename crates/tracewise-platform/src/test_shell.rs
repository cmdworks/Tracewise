use super::*;

#[test]
fn shell_blocks_zsh() {
    let block = zsh_block(false);
    assert!(block.contains("add-zsh-hook preexec _tracewise_preexec"));
    assert!(block.contains("add-zsh-hook precmd _tracewise_precmd"));
    assert!(block.contains("_tracewise_shell_id"));
}

#[test]
fn shell_blocks_bash() {
    let block = bash_block(false);
    assert!(block.contains("PROMPT_COMMAND"));
    assert!(block.contains("tracewise shell precmd"));
}

#[test]
fn shell_blocks_powershell() {
    let block = powershell_block();
    assert!(block.contains("Invoke-TracewisePrompt"));
    assert!(block.contains("$env:TRACEWISE_SHELL_ACTIVE"));
    assert!(block.contains("& tracewise shell precmd"));
}

#[test]
fn shell_blocks_non_interactive() {
    let zsh_ni = zsh_block(true);
    assert!(zsh_ni.contains("TRACEWISE_SHELL_ACTIVE=1"));
    assert!(zsh_ni.contains("shell init zsh --non-interactive"));
    let bash_ni = bash_block(true);
    assert!(bash_ni.contains("TRACEWISE_SHELL_ACTIVE=1"));
    assert!(bash_ni.contains("shell init bash --non-interactive"));
}

#[test]
fn the_fish_snippet_uses_fishs_own_events_and_reads_the_exit_status_first() {
    let fish = fish_interactive();
    assert!(fish.contains("--on-event fish_preexec") && fish.contains("--on-event fish_postexec"));
    assert!(fish.contains("tracewise shell preexec") && fish.contains("tracewise shell precmd"));
    let post = fish.split("fish_postexec").nth(1).unwrap();
    assert!(
        post.find("set -l code $status").unwrap() < post.find("tracewise shell precmd").unwrap(),
        "status read before any other command"
    );
}

#[test]
fn pwsh_and_nu_snippets_call_both_hooks() {
    for snippet in [pwsh_interactive(), nu_interactive()] {
        assert!(snippet.contains("tracewise shell preexec --id"));
        assert!(snippet.contains("tracewise shell precmd --id"));
    }
    assert!(pwsh_interactive().contains("Set-PSReadLineKeyHandler -Key Enter"));
    assert!(nu_interactive().contains("pre_execution") && nu_interactive().contains("pre_prompt"));
}
