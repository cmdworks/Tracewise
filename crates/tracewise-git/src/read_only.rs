//! Read-only git command detection.

/// Returns the git subcommand name from arguments, skipping global git options.
pub fn find_subcommand<T: AsRef<str>>(args: &[T]) -> Option<(&str, usize)> {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_ref();
        if arg == "-C" || arg == "-c" || arg == "--git-dir" || arg == "--work-tree" || arg == "--namespace" || arg == "--exec-path" {
            i += 2;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        return Some((arg, i));
    }
    None
}

/// Returns true if the git invocation is a work-tree changing command that Tracewise records:
/// checkout, switch, reset, restore, merge, rebase, pull, stash (push/pop/apply/drop/clear/branch),
/// commit, apply, clean, cherry-pick, revert, am, mv, rm, sparse-checkout, submodule, bisect, read-tree, checkout-index.
pub fn is_worktree_mutating_git<T: AsRef<str>>(args: &[T]) -> bool {
    let (sub, idx) = match find_subcommand(args) {
        Some(s) => s,
        None => return false,
    };
    match sub {
        "checkout" | "switch" | "reset" | "restore" | "merge" | "rebase" | "pull" | "commit" | "apply" | "clean" | "cherry-pick"
        | "revert" | "am" | "mv" | "rm" | "sparse-checkout" | "submodule" | "bisect" | "read-tree" | "checkout-index" => true,
        "stash" => {
            // stash with 'list' or 'show' is read-only; all other stash actions mutate
            let mut i = idx + 1;
            while i < args.len() {
                let a = args[i].as_ref();
                if a.starts_with('-') {
                    i += 1;
                    continue;
                }
                if a == "list" || a == "show" {
                    return false;
                }
                break;
            }
            true
        }
        _ => false,
    }
}

/// Returns true if the git invocation is read-only (e.g. status, rev-parse, log, diff,
/// for-each-ref, stash list, worktree list, branch, config, etc.).
pub fn is_read_only_git<T: AsRef<str>>(args: &[T]) -> bool {
    let (sub, idx) = match find_subcommand(args) {
        Some(s) => s,
        None => return true,
    };
    if sub == "clone" {
        return false;
    }
    if sub == "worktree" {
        let mut i = idx + 1;
        while i < args.len() {
            let a = args[i].as_ref();
            if a.starts_with('-') {
                i += 1;
                continue;
            }
            if a == "add" {
                return false;
            }
            break;
        }
        return true;
    }
    !is_worktree_mutating_git(args)
}

#[cfg(test)]
#[path = "test_read_only.rs"]
mod tests;
