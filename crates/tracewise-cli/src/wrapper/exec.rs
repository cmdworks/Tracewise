//! Git binary location, argument inspection, and read-only fast path.
use super::*;

pub(crate) fn is_tracewise_bin_name(name: &str) -> bool {
    name == "tracewise"
        || name == "tracewise.exe"
        || name == "tracewise-git"
        || name == "tracewise-git.exe"
}

pub fn find_real_git_inner(path_var: Option<&str>, wrapper_dir: Option<&Path>) -> PathBuf {
    if let Ok(p) = std::env::var("TRACEWISE_REAL_GIT") {
        let pb = PathBuf::from(&p);
        if pb.is_file() {
            return pb;
        }
    }
    // Check /usr/bin/git first
    let usr_git = Path::new("/usr/bin/git");
    if usr_git.is_file() {
        let is_wrapper = if let Some(w) = wrapper_dir { usr_git.parent() == Some(w) } else { false };
        if !is_wrapper {
            return PathBuf::from("/usr/bin/git");
        }
    }
    if let Some(path_str) = path_var {
        let self_exe = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
        for dir in std::env::split_paths(path_str) {
            if let Some(w) = wrapper_dir {
                if dir == w {
                    continue;
                }
            }
            let candidate = dir.join("git");
            if candidate.is_file() {
                if let Ok(canon) = candidate.canonicalize() {
                    if let Some(ref self_p) = self_exe {
                        if &canon == self_p {
                            continue;
                        }
                    }
                    if let Some(name) = canon.file_name().and_then(|n| n.to_str()) {
                        if is_tracewise_bin_name(name) {
                            continue;
                        }
                    }
                }
                if let Ok(target) = fs::read_link(&candidate) {
                    if let Some(name) = target.file_name().and_then(|n| n.to_str()) {
                        if is_tracewise_bin_name(name) {
                            continue;
                        }
                    }
                }
                return candidate;
            }
        }
    }
    PathBuf::from("/usr/bin/git")
}

pub fn find_real_git(wrapper_dir: Option<&Path>) -> PathBuf {
    let path_val = std::env::var("PATH").ok();
    find_real_git_inner(path_val.as_deref(), wrapper_dir)
}

pub fn find_clone_target(git_args: &[String]) -> Option<PathBuf> {
    let mut idx = 0;
    while idx < git_args.len() {
        let arg = &git_args[idx];
        if arg == "clone" {
            idx += 1;
            break;
        }
        idx += 1;
    }

    let mut non_flags = Vec::new();
    while idx < git_args.len() {
        let arg = &git_args[idx];
        if arg == "-b"
            || arg == "--branch"
            || arg == "-u"
            || arg == "--upload-pack"
            || arg == "--reference"
            || arg == "--reference-if-able"
            || arg == "--depth"
            || arg == "--shallow-since"
            || arg == "--shallow-exclude"
            || arg == "--origin"
            || arg == "-o"
            || arg == "-c"
            || arg == "--config"
            || arg == "--separate-git-dir"
            || arg == "--template"
        {
            idx += 2;
            continue;
        } else if arg.starts_with('-') {
            idx += 1;
            continue;
        } else {
            non_flags.push(arg.clone());
            idx += 1;
        }
    }

    if non_flags.is_empty() {
        return None;
    }

    if non_flags.len() >= 2 {
        Some(PathBuf::from(&non_flags[1]))
    } else {
        let repo = &non_flags[0];
        let trimmed = repo.trim_end_matches('/');
        let name = Path::new(trimmed).file_name()?.to_str()?;
        let name = name.strip_suffix(".git").unwrap_or(name);
        Some(PathBuf::from(name))
    }
}

pub(crate) fn is_working_tree_changing_subcmd(sub: &str) -> bool {
    matches!(
        sub,
        "checkout"
            | "switch"
            | "reset"
            | "restore"
            | "merge"
            | "rebase"
            | "pull"
            | "stash"
            | "commit"
            | "apply"
            | "clean"
            | "cherry-pick"
            | "revert"
            | "am"
            | "mv"
            | "rm"
            | "sparse-checkout"
            | "submodule"
            | "bisect"
            | "read-tree"
            | "checkout-index"
    )
}

pub(crate) fn fast_read_only_git_exec(git_args: &[String]) {
    let real_git = find_real_git(None);
    tracewise_platform::process::fast_exec(&real_git, git_args, Some("git"), None, None);
}

pub(crate) fn handle_git_depth_fallback(git_args: &[String]) {
    if std::env::var("TRACEWISE_GIT_DEPTH").is_ok() {
        let fallback = Path::new("/usr/bin/git");
        let target = if fallback.is_file() { fallback } else { Path::new("git") };
        tracewise_platform::process::fast_exec(target, git_args, None, None, None);
    }
}

pub(crate) struct GitPreState {
    pub old_head: String,
    pub now: String,
    pub dirty_before: Vec<PathBuf>,
}

pub(crate) fn prepare_git_pre_state(real_git: &Path, git_args: &[String]) -> GitPreState {
    let current_dir_before = std::env::current_dir().ok();
    let repo_paths_before = current_dir_before.as_ref().and_then(|d| RepoPaths::discover(d));
    let (_old_branch, old_head) = if let Some(ref rp) = repo_paths_before { get_git_info(rp) } else { (String::new(), String::new()) };

    check_commit_gate(&repo_paths_before, git_args);

    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let dirty_before: Vec<PathBuf> = match &repo_paths_before {
        Some(rp) if git_args.iter().find(|a| !a.starts_with('-')).is_some_and(|s| is_working_tree_changing_subcmd(s)) => {
            dirty_paths(real_git, &rp.work_tree)
        }
        _ => Vec::new(),
    };

    GitPreState { old_head, now, dirty_before }
}
