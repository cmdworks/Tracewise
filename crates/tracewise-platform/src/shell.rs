//! Shell integration snippet generators for zsh, bash, and PowerShell.

/// Interactive snippet for zsh.
pub fn zsh_interactive() -> &'static str {
    "# Tracewise shell integration for zsh\n\
     autoload -Uz add-zsh-hook\n\
     _tracewise_shell_id=\"${_tracewise_shell_id:-$$}\"\n\n\
     _tracewise_preexec() {\n    \
         tracewise shell preexec --id \"$_tracewise_shell_id\" -- \"$1\"\n\
     }\n\n\
     _tracewise_precmd() {\n    \
         local code=$?\n    \
         tracewise shell precmd --id \"$_tracewise_shell_id\" --exit \"$code\"\n\
     }\n\n\
     add-zsh-hook preexec _tracewise_preexec\n\
     add-zsh-hook precmd _tracewise_precmd\n"
}

/// Interactive snippet for fish: fish's own events run `tracewise shell preexec` and `precmd`, like zsh's hooks.
pub fn fish_interactive() -> &'static str {
    r#"# Tracewise shell integration for fish
set -q _tracewise_shell_id; or set -g _tracewise_shell_id %self

function _tracewise_preexec --on-event fish_preexec
    tracewise shell preexec --id $_tracewise_shell_id -- "$argv[1]" 2>/dev/null; or true
end

function _tracewise_postexec --on-event fish_postexec
    set -l code $status
    tracewise shell precmd --id $_tracewise_shell_id --exit $code 2>/dev/null; or true
end
"#
}

/// Interactive snippet for PowerShell: PSReadLine's Enter handler is the pre-execution hook, the prompt function the post one.
pub fn pwsh_interactive() -> &'static str {
    r#"# Tracewise shell integration for PowerShell (needs PSReadLine, which ships with PowerShell 7)
if (-not $global:_tracewise_shell_id) { $global:_tracewise_shell_id = $PID }

Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
    $line = $null; $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    if ($line) { & tracewise shell preexec --id $global:_tracewise_shell_id -- $line 2>$null }
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
}

$global:_tracewise_prompt = $function:prompt
function global:prompt {
    $ok = $?
    $code = if ($ok) { 0 } elseif ($LASTEXITCODE) { $LASTEXITCODE } else { 1 }
    & tracewise shell precmd --id $global:_tracewise_shell_id --exit $code 2>$null
    & $global:_tracewise_prompt
}
"#
}

/// Interactive snippet for nushell: the `pre_execution` and `pre_prompt` hooks.
pub fn nu_interactive() -> &'static str {
    r#"# Tracewise shell integration for nushell
$env._tracewise_shell_id = ($env._tracewise_shell_id? | default (random int))
$env.config = ($env.config | upsert hooks.pre_execution (
    ($env.config.hooks.pre_execution? | default []) | append {||
        ^tracewise shell preexec --id $env._tracewise_shell_id -- (commandline) | ignore
    }
))
$env.config = ($env.config | upsert hooks.pre_prompt (
    ($env.config.hooks.pre_prompt? | default []) | append {||
        ^tracewise shell precmd --id $env._tracewise_shell_id --exit $env.LAST_EXIT_CODE | ignore
    }
))
"#
}

/// Non-interactive block for ~/.zshenv.
pub fn zsh_non_interactive(tracewise_bin: &str) -> String {
    format!(
        "# Tracewise non-interactive shell integration for zsh\n\
         if [[ ! -o interactive ]] && [[ -z \"$TRACEWISE_SHELL_ACTIVE\" ]] && [[ -x \"{tracewise_bin}\" ]]; then\n\
             export TRACEWISE_SHELL_ACTIVE=1\n\
             \"{tracewise_bin}\" shell begin --pid $$ -- \"${{ZSH_EXECUTION_STRING:-$*}}\" || exit 1\n\
             TRAPEXIT() {{\n\
                 local code=$?\n\
                 \"{tracewise_bin}\" shell end --pid $$ --exit $code\n\
                 return $code\n\
             }}\n\
         fi\n"
    )
}

/// Non-interactive file for BASH_ENV.
pub fn bash_non_interactive(tracewise_bin: &str) -> String {
    format!(
        "# Tracewise non-interactive shell integration for bash\n\
         if [ -z \"$PS1\" ] && [ -z \"$TRACEWISE_SHELL_ACTIVE\" ] && [ -x \"{tracewise_bin}\" ]; then\n\
             export TRACEWISE_SHELL_ACTIVE=1\n\
             \"{tracewise_bin}\" shell begin --pid $$ -- \"${{BASH_EXECUTION_STRING:-$BASH_COMMAND}}\" || exit 1\n\
             trap '\"{tracewise_bin}\" shell end --pid $$ --exit $?' EXIT\n\
         fi\n"
    )
}

/// Generate the zsh shell integration snippet.
pub fn zsh_block(non_interactive: bool) -> String {
    if non_interactive {
        r#"# >>> tracewise shell block >>>
if [ -z "$TRACEWISE_SHELL_ACTIVE" ]; then
    export TRACEWISE_SHELL_ACTIVE=1
    if [ -x "${TRACEWISE_BIN:-$HOME/.tracewise/bin/tracewise}" ]; then
        eval "$("${TRACEWISE_BIN:-$HOME/.tracewise/bin/tracewise}" shell init zsh --non-interactive 2>/dev/null)"
    fi
fi
# <<< tracewise shell block <<<
"#
        .to_string()
    } else {
        r#"# >>> tracewise shell block >>>
if [ -z "$TRACEWISE_SHELL_ACTIVE" ]; then
    autoload -Uz add-zsh-hook
    _tracewise_shell_id="${_tracewise_shell_id:-$$}"
    _tracewise_preexec() {
        tracewise shell preexec --id "$_tracewise_shell_id" -- "$1" 2>/dev/null || true
    }
    _tracewise_precmd() {
        tracewise shell precmd --id "$_tracewise_shell_id" --exit "$?" 2>/dev/null || true
    }
    add-zsh-hook preexec _tracewise_preexec
    add-zsh-hook precmd _tracewise_precmd
fi
# <<< tracewise shell block <<<
"#
        .to_string()
    }
}

/// Generate the bash shell integration snippet.
pub fn bash_block(non_interactive: bool) -> String {
    if non_interactive {
        r#"# >>> tracewise shell block >>>
if [ -z "$TRACEWISE_SHELL_ACTIVE" ]; then
    export TRACEWISE_SHELL_ACTIVE=1
    if [ -x "${TRACEWISE_BIN:-$HOME/.tracewise/bin/tracewise}" ]; then
        eval "$("${TRACEWISE_BIN:-$HOME/.tracewise/bin/tracewise}" shell init bash --non-interactive 2>/dev/null)"
    fi
fi
# <<< tracewise shell block <<<
"#
        .to_string()
    } else {
        r#"# >>> tracewise shell block >>>
if [ -z "$TRACEWISE_SHELL_ACTIVE" ]; then
    _tracewise_shell_id="${_tracewise_shell_id:-$$}"
    _tracewise_prompt_command() {
        local exit_code=$?
        tracewise shell precmd --id "$_tracewise_shell_id" --exit "$exit_code" 2>/dev/null || true
    }
    PROMPT_COMMAND="_tracewise_prompt_command;${PROMPT_COMMAND:-}"
fi
# <<< tracewise shell block <<<
"#
        .to_string()
    }
}

/// Generate the PowerShell integration snippet.
pub fn powershell_block() -> String {
    r#"# >>> tracewise shell block >>>
if (-not $env:TRACEWISE_SHELL_ACTIVE) {
    $env:TRACEWISE_SHELL_ACTIVE = "1"
    function Invoke-TracewisePrompt {
        $lastExit = $LASTEXITCODE
        & tracewise shell precmd --id $PID --exit $lastExit 2>$null
    }
}
# <<< tracewise shell block <<<
"#
    .to_string()
}

#[cfg(test)]
#[path = "test_shell.rs"]
mod tests;
