use std::path::PathBuf;

/// Predicts files that will be written by common shell commands.
pub fn predict_written_files(command: &str) -> Vec<PathBuf> {
    predict_inner(command, 0)
}

fn predict_inner(command: &str, depth: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // Split on command separators (;, &&, ||, |)
    for part in split_commands(command) {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check for redirection: > or >>
        if let Some(pos) = last_unquoted_redirect(trimmed) {
            let after = &trimmed[pos + 1..];
            let target = after.trim_start_matches('>').trim();
            let first_word = target.split_whitespace().next().unwrap_or("");
            let clean = clean_path(first_word);
            if !clean.is_empty() {
                files.push(PathBuf::from(clean));
                continue;
            }
        }

        // Split command into tokens (respecting quotes)
        let tokens = tokenize(trimmed);
        if tokens.is_empty() {
            continue;
        }

        let tokens = strip_wrappers(tokens);
        if tokens.is_empty() {
            continue;
        }
        let prog_path = tokens[0].clone();
        let prog = prog_path.rsplit('/').next().unwrap_or(&prog_path);

        predict_token_files(prog, &tokens, depth, &mut files);
    }

    files
}

fn predict_token_files(prog: &str, tokens: &[String], depth: usize, files: &mut Vec<PathBuf>) {
    match prog {
        "sh" | "bash" | "zsh" | "dash" | "ksh" if depth < 3 => {
            if let Some(script) = shell_c_script(tokens) {
                files.extend(predict_inner(&script, depth + 1));
            }
        }
        "chflags" | "chmod" | "chown" | "chgrp" => {
            let operands: Vec<&String> = tokens[1..].iter().filter(|t| !t.starts_with('-')).collect();
            for arg in operands.into_iter().skip(1) {
                files.push(PathBuf::from(clean_path(arg)));
            }
        }
        "xattr" => {
            let flags: String =
                tokens[1..].iter().filter(|t| t.starts_with('-') && !t.starts_with("--")).flat_map(|t| t[1..].chars()).collect();
            let operands: Vec<&String> = tokens[1..].iter().filter(|t| !t.starts_with('-')).collect();
            let skip = if flags.contains('w') {
                2
            } else if flags.contains('d') {
                1
            } else if flags.contains('c') {
                0
            } else {
                usize::MAX
            };
            if skip != usize::MAX {
                for arg in operands.into_iter().skip(skip) {
                    files.push(PathBuf::from(clean_path(arg)));
                }
            }
        }
        "touch" | "rm" | "tee" => {
            for arg in &tokens[1..] {
                if !arg.starts_with('-') {
                    let clean = clean_path(arg);
                    if !clean.is_empty() {
                        files.push(PathBuf::from(clean));
                    }
                }
            }
        }
        "cp" => {
            let non_flags: Vec<&String> = tokens[1..].iter().filter(|t| !t.starts_with('-')).collect();
            if let Some(dest) = non_flags.last() {
                let clean = clean_path(dest);
                if !clean.is_empty() {
                    files.push(PathBuf::from(clean));
                }
            }
        }
        "mv" => {
            let non_flags: Vec<&String> = tokens[1..].iter().filter(|t| !t.starts_with('-')).collect();
            for arg in non_flags {
                let clean = clean_path(arg);
                if !clean.is_empty() {
                    files.push(PathBuf::from(clean));
                }
            }
        }
        "sed" | "perl" => {
            if tokens.iter().any(|t| t == "-i" || t.starts_with("-i")) {
                let non_flags: Vec<&String> = tokens[1..].iter().filter(|t| !t.starts_with('-')).collect();
                if non_flags.len() >= 2 {
                    if let Some(file) = non_flags.last() {
                        let clean = clean_path(file);
                        if !clean.is_empty() {
                            files.push(PathBuf::from(clean));
                        }
                    }
                }
            }
        }
        "git" if tokens.len() >= 2 => {
            let sub = tokens[1].as_str();
            if sub == "checkout" || sub == "restore" {
                let mut past_double_dash = false;
                for arg in &tokens[2..] {
                    if arg == "--" {
                        past_double_dash = true;
                        continue;
                    }
                    if past_double_dash || !arg.starts_with('-') {
                        let clean = clean_path(arg);
                        if !clean.is_empty() {
                            files.push(PathBuf::from(clean));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Byte index of the last `>` that is outside quotes and is not part of `>&` (as in `2>&1`).
fn last_unquoted_redirect(s: &str) -> Option<usize> {
    let mut in_quote = None;
    let mut last = None;
    let mut chars = s.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        match ch {
            '\'' | '"' if in_quote == Some(ch) => in_quote = None,
            '\'' | '"' if in_quote.is_none() => in_quote = Some(ch),
            '>' if in_quote.is_none() && chars.peek().map(|(_, c)| *c) != Some('&') => last = Some(i),
            _ => {}
        }
    }
    last
}

fn split_commands(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = None;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\'' | '"' => {
                if in_quote == Some(ch) {
                    in_quote = None;
                } else if in_quote.is_none() {
                    in_quote = Some(ch);
                }
                current.push(ch);
            }
            ';' | '|' if in_quote.is_none() => {
                if !current.trim().is_empty() {
                    parts.push(current);
                    current = String::new();
                }
                if ch == '|' && i + 1 < chars.len() && chars[i + 1] == '|' {
                    i += 1; // skip second '|' of '||'
                }
            }
            '&' if in_quote.is_none() && i + 1 < chars.len() && chars[i + 1] == '&' => {
                if !current.trim().is_empty() {
                    parts.push(current);
                    current = String::new();
                }
                i += 1; // skip second '&'
            }
            _ => {
                current.push(ch);
            }
        }
        i += 1;
    }

    if !current.trim().is_empty() {
        parts.push(current);
    }

    parts
}

/// The script text after `-c` in `sh -c "..."`.
fn shell_c_script(tokens: &[String]) -> Option<String> {
    let at = tokens.iter().position(|t| t == "-c" || (t.starts_with('-') && !t.starts_with("--") && t.ends_with('c')))?;
    tokens.get(at + 1).cloned()
}

const INTERPRETERS: [&str; 9] = ["python", "python3", "node", "deno", "bun", "perl", "ruby", "php", "osascript"];
const INLINE_CODE_FLAGS: [&str; 7] = ["-c", "-e", "-E", "-p", "-r", "--eval", "--print"];
const WRITERS: [&str; 13] = ["sed", "tee", "mv", "cp", "rm", "touch", "truncate", "ln", "patch", "chmod", "chown", "chflags", "xattr"];

const READERS: &[&str] = &[
    "ls", "cat", "sleep", "pwd", "echo", "printf", "head", "tail", "wc", "grep", "rg", "sort", "uniq", "which", "whoami", "date", "true",
    "false", "test", "[", "diff", "cmp", "stat", "file", "du", "df", "ps", "pgrep", "uname", "sw_vers", "cd", "id", "hostname", "tree",
    "less", "more",
];
const READ_ONLY_GIT: &[&str] = &["status", "log", "diff", "show", "branch", "rev-parse", "ls-files", "blame", "remote"];
const READ_ONLY_TRAIL: &[&str] = &["log", "why", "blame", "status", "pending", "doctor", "verify", "commands", "show", "ps"];

/// True when every simple command in `command` is a known reader (`ls`, `sleep`, `git status`, `trail log` ...) with no
/// output redirection: nothing it does can change a file, so a change that appears while it runs is not its.
pub fn only_reads(command: &str) -> bool {
    if last_unquoted_redirect(command).is_some() {
        return false;
    }
    let parts = split_commands(command);
    !parts.is_empty()
        && parts.iter().all(|part| {
            let tokens = strip_wrappers(tokenize(part.trim()));
            let Some(first) = tokens.first() else {
                return true;
            };
            let prog = base_name(first);
            let sub = tokens.iter().skip(1).find(|t| !t.starts_with('-')).map(String::as_str);
            match prog.as_str() {
                // an interpreter only reads when it merely prints its version
                "node" | "python3" => tokens.iter().any(|t| t == "--version" || t == "-V"),
                "git" => sub.is_some_and(|s| READ_ONLY_GIT.contains(&s)),
                // `trail run --why "..." -- <command>`: it reads when the command after `--` does
                "trail" if sub == Some("run") => {
                    tokens.iter().position(|t| t == "--").is_some_and(|i| only_reads(&tokens[i + 1..].join(" ")))
                }
                "trail" => sub.is_some_and(|s| READ_ONLY_TRAIL.contains(&s)),
                p => READERS.contains(&p),
            }
        })
}

/// True for commands whose target files cannot be read off the command line: an interpreter running inline code,
/// `xargs` feeding a writer, and anything that starts work that outlives the command (`&`, nohup, launchctl, crontab, at).
/// An agent needs a declared reason for these even though no path is predicted.
pub fn may_write_anywhere(command: &str) -> bool {
    may_write_anywhere_inner(command, 0)
}

fn may_write_anywhere_inner(command: &str, depth: usize) -> bool {
    for part in split_commands(command) {
        let trimmed = part.trim();
        if part_starts_background(trimmed) {
            return true;
        }
        let tokens = strip_wrappers(tokenize(trimmed));
        let Some(first) = tokens.first() else {
            continue;
        };
        let prog = base_name(first);
        let rest = &tokens[1..];
        match prog.as_str() {
            "xargs" => {
                let inner = rest.iter().skip_while(|t| t.starts_with('-'));
                if inner.map(|t| base_name(t)).next().is_some_and(|p| WRITERS.contains(&p.as_str())) {
                    return true;
                }
            }
            "sh" | "bash" | "zsh" | "dash" | "ksh" if depth < 3 => {
                if shell_c_script(&tokens).is_some_and(|sc| may_write_anywhere_inner(&sc, depth + 1)) {
                    return true;
                }
            }
            p if INTERPRETERS.iter().any(|i| p == *i || p.starts_with(&format!("{i}3.")))
                && rest.iter().any(|t| INLINE_CODE_FLAGS.contains(&t.as_str())) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn base_name(t: &str) -> String {
    t.rsplit('/').next().unwrap_or(t).to_string()
}

/// One simple command that starts work which outlives it: `&`, nohup, disown, setsid, at, launchctl, crontab.
fn part_starts_background(trimmed: &str) -> bool {
    if trimmed.ends_with('&') && !trimmed.ends_with("&&") {
        return true;
    }
    let raw = tokenize(trimmed);
    let Some(first) = raw.first() else {
        return false;
    };
    if matches!(base_name(first).as_str(), "nohup" | "disown" | "setsid" | "at" | "batch") {
        return true;
    }
    let tokens = strip_wrappers(raw);
    let Some(first) = tokens.first() else {
        return false;
    };
    let rest = &tokens[1..];
    let lctl_cmd = tracewise_platform::service::LAUNCHCTL;
    match base_name(first).as_str() {
        cmd if cmd == lctl_cmd => !rest.iter().any(|t| matches!(t.as_str(), "print" | "list" | "blame" | "error" | "version" | "help")),
        "crontab" => !rest.iter().any(|t| t == "-l"),
        _ => false,
    }
}

/// True when the command starts something that keeps running after it ends (see `part_starts_background`).
/// `sh -c "…"` is judged by its script.
pub fn starts_background(command: &str) -> bool {
    fn inner(command: &str, depth: usize) -> bool {
        split_commands(command).iter().any(|part| {
            let trimmed = part.trim();
            if part_starts_background(trimmed) {
                return true;
            }
            let tokens = strip_wrappers(tokenize(trimmed));
            depth < 3
                && tokens.first().is_some_and(|f| matches!(base_name(f).as_str(), "sh" | "bash" | "zsh" | "dash" | "ksh"))
                && shell_c_script(&tokens).is_some_and(|sc| inner(&sc, depth + 1))
        })
    }
    inner(command, 0)
}

/// The simple commands of `command` as word lists, launchers (`sudo`, `env`, …) removed.
pub fn command_words(command: &str) -> Vec<Vec<String>> {
    split_commands(command).iter().map(|p| strip_wrappers(tokenize(p.trim()))).filter(|w| !w.is_empty()).collect()
}

/// Drops launchers that do not change what is written: `sudo`, `command`, `env VAR=x`, `exec`, `nohup`, `nice`, `time`.
fn strip_wrappers(mut tokens: Vec<String>) -> Vec<String> {
    loop {
        let Some(first) = tokens.first() else {
            return tokens;
        };
        let name = first.rsplit('/').next().unwrap_or(first).to_string();
        match name.as_str() {
            "sudo" | "command" | "exec" | "nohup" | "nice" | "time" | "env" => {
                tokens.remove(0);
                while tokens.first().is_some_and(|t| t.starts_with('-') || (name == "env" && t.contains('='))) {
                    tokens.remove(0);
                }
            }
            _ => return tokens,
        }
    }
}

fn clean_path(s: &str) -> String {
    s.trim_matches(|c| c == '\'' || c == '"' || c == ';' || c == '&').to_string()
}

fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = None;

    for ch in s.chars() {
        match ch {
            '\'' | '"' => {
                if in_quote == Some(ch) {
                    in_quote = None;
                } else if in_quote.is_none() {
                    in_quote = Some(ch);
                } else {
                    current.push(ch);
                }
            }
            c if c.is_whitespace() && in_quote.is_none() => {
                if !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
#[path = "test_predict.rs"]
mod tests;
