//! Shell integration snippet generators for zsh, bash, and PowerShell.

/// Interactive snippet for zsh.
pub fn zsh_interactive() -> &'static str {
    "# Trail shell integration for zsh\n\
     autoload -Uz add-zsh-hook\n\
     _trail_shell_id=\"${_trail_shell_id:-$$}\"\n\n\
     _trail_preexec() {\n    \
         trail shell preexec --id \"$_trail_shell_id\" -- \"$1\"\n\
     }\n\n\
     _trail_precmd() {\n    \
         local code=$?\n    \
         trail shell precmd --id \"$_trail_shell_id\" --exit \"$code\"\n\
     }\n\n\
     add-zsh-hook preexec _trail_preexec\n\
     add-zsh-hook precmd _trail_precmd\n"
}

/// Interactive snippet for fish: fish's own events run `trail shell preexec` and `precmd`, like zsh's hooks.
pub fn fish_interactive() -> &'static str {
    r#"# Trail shell integration for fish
set -q _trail_shell_id; or set -g _trail_shell_id %self

function _trail_preexec --on-event fish_preexec
    trail shell preexec --id $_trail_shell_id -- "$argv[1]" 2>/dev/null; or true
end

function _trail_postexec --on-event fish_postexec
    set -l code $status
    trail shell precmd --id $_trail_shell_id --exit $code 2>/dev/null; or true
end
"#
}

/// Interactive snippet for PowerShell: PSReadLine's Enter handler is the pre-execution hook, the prompt function the post one.
pub fn pwsh_interactive() -> &'static str {
    r#"# Trail shell integration for PowerShell (needs PSReadLine, which ships with PowerShell 7)
if (-not $global:_trail_shell_id) { $global:_trail_shell_id = $PID }

Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
    $line = $null; $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    if ($line) { & trail shell preexec --id $global:_trail_shell_id -- $line 2>$null }
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
}

$global:_trail_prompt = $function:prompt
function global:prompt {
    $ok = $?
    $code = if ($ok) { 0 } elseif ($LASTEXITCODE) { $LASTEXITCODE } else { 1 }
    & trail shell precmd --id $global:_trail_shell_id --exit $code 2>$null
    & $global:_trail_prompt
}
"#
}

/// Interactive snippet for nushell: the `pre_execution` and `pre_prompt` hooks.
pub fn nu_interactive() -> &'static str {
    r#"# Trail shell integration for nushell
$env._trail_shell_id = ($env._trail_shell_id? | default (random int))
$env.config = ($env.config | upsert hooks.pre_execution (
    ($env.config.hooks.pre_execution? | default []) | append {||
        ^trail shell preexec --id $env._trail_shell_id -- (commandline) | ignore
    }
))
$env.config = ($env.config | upsert hooks.pre_prompt (
    ($env.config.hooks.pre_prompt? | default []) | append {||
        ^trail shell precmd --id $env._trail_shell_id --exit $env.LAST_EXIT_CODE | ignore
    }
))
"#
}

/// Non-interactive block for ~/.zshenv.
pub fn zsh_non_interactive(trail_bin: &str) -> String {
    format!(
        "# Trail non-interactive shell integration for zsh\n\
         if [[ ! -o interactive ]] && [[ -z \"$TRAIL_SHELL_ACTIVE\" ]] && [[ -x \"{trail_bin}\" ]]; then\n\
             export TRAIL_SHELL_ACTIVE=1\n\
             \"{trail_bin}\" shell begin --pid $$ -- \"${{ZSH_EXECUTION_STRING:-$*}}\" || exit 1\n\
             TRAPEXIT() {{\n\
                 local code=$?\n\
                 \"{trail_bin}\" shell end --pid $$ --exit $code\n\
                 return $code\n\
             }}\n\
         fi\n"
    )
}

/// Non-interactive file for BASH_ENV.
pub fn bash_non_interactive(trail_bin: &str) -> String {
    format!(
        "# Trail non-interactive shell integration for bash\n\
         if [ -z \"$PS1\" ] && [ -z \"$TRAIL_SHELL_ACTIVE\" ] && [ -x \"{trail_bin}\" ]; then\n\
             export TRAIL_SHELL_ACTIVE=1\n\
             \"{trail_bin}\" shell begin --pid $$ -- \"${{BASH_EXECUTION_STRING:-$BASH_COMMAND}}\" || exit 1\n\
             trap '\"{trail_bin}\" shell end --pid $$ --exit $?' EXIT\n\
         fi\n"
    )
}

/// Generate the zsh shell integration snippet.
pub fn zsh_block(non_interactive: bool) -> String {
    if non_interactive {
        r#"# >>> trail shell block >>>
if [ -z "$TRAIL_SHELL_ACTIVE" ]; then
    export TRAIL_SHELL_ACTIVE=1
    if [ -x "${TRAIL_BIN:-$HOME/.trail/bin/trail}" ]; then
        eval "$("${TRAIL_BIN:-$HOME/.trail/bin/trail}" shell init zsh --non-interactive 2>/dev/null)"
    fi
fi
# <<< trail shell block <<<
"#
        .to_string()
    } else {
        r#"# >>> trail shell block >>>
if [ -z "$TRAIL_SHELL_ACTIVE" ]; then
    autoload -Uz add-zsh-hook
    _trail_shell_id="${_trail_shell_id:-$$}"
    _trail_preexec() {
        trail shell preexec --id "$_trail_shell_id" -- "$1" 2>/dev/null || true
    }
    _trail_precmd() {
        trail shell precmd --id "$_trail_shell_id" --exit "$?" 2>/dev/null || true
    }
    add-zsh-hook preexec _trail_preexec
    add-zsh-hook precmd _trail_precmd
fi
# <<< trail shell block <<<
"#
        .to_string()
    }
}

/// Generate the bash shell integration snippet.
pub fn bash_block(non_interactive: bool) -> String {
    if non_interactive {
        r#"# >>> trail shell block >>>
if [ -z "$TRAIL_SHELL_ACTIVE" ]; then
    export TRAIL_SHELL_ACTIVE=1
    if [ -x "${TRAIL_BIN:-$HOME/.trail/bin/trail}" ]; then
        eval "$("${TRAIL_BIN:-$HOME/.trail/bin/trail}" shell init bash --non-interactive 2>/dev/null)"
    fi
fi
# <<< trail shell block <<<
"#
        .to_string()
    } else {
        r#"# >>> trail shell block >>>
if [ -z "$TRAIL_SHELL_ACTIVE" ]; then
    _trail_shell_id="${_trail_shell_id:-$$}"
    _trail_prompt_command() {
        local exit_code=$?
        trail shell precmd --id "$_trail_shell_id" --exit "$exit_code" 2>/dev/null || true
    }
    PROMPT_COMMAND="_trail_prompt_command;${PROMPT_COMMAND:-}"
fi
# <<< trail shell block <<<
"#
        .to_string()
    }
}

/// Generate the PowerShell integration snippet.
pub fn powershell_block() -> String {
    r#"# >>> trail shell block >>>
if (-not $env:TRAIL_SHELL_ACTIVE) {
    $env:TRAIL_SHELL_ACTIVE = "1"
    function Invoke-TrailPrompt {
        $lastExit = $LASTEXITCODE
        & trail shell precmd --id $PID --exit $lastExit 2>$null
    }
}
# <<< trail shell block <<<
"#
    .to_string()
}

#[cfg(test)]
#[path = "test_shell.rs"]
mod tests;
