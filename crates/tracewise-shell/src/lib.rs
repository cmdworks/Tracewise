//! Shell integration scripts and hooks for Tracewise.

/// Interactive snippet for zsh.
pub fn zsh_interactive() -> &'static str {
    tracewise_platform::shell::zsh_interactive()
}

/// Interactive snippet for fish.
pub fn fish_interactive() -> &'static str {
    tracewise_platform::shell::fish_interactive()
}

/// Non-interactive block for ~/.zshenv.
pub fn zsh_non_interactive(tracewise_bin: &str) -> String {
    tracewise_platform::shell::zsh_non_interactive(tracewise_bin)
}

/// Non-interactive file for BASH_ENV.
pub fn bash_non_interactive(tracewise_bin: &str) -> String {
    tracewise_platform::shell::bash_non_interactive(tracewise_bin)
}

#[cfg(test)]
#[path = "test_lib.rs"]
mod tests;
