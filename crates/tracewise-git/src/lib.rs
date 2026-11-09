//! Git interaction, repository discovery, and ignore handling for Tracewise.

pub mod read_only;
pub use read_only::{is_read_only_git, is_worktree_mutating_git};

use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};

/// Paths resolved for a discovered git repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPaths {
    /// The root of the working tree (e.g. /path/to/repo or /path/to/worktree)
    pub work_tree: PathBuf,
    /// The .git directory for this work tree
    pub git_dir: PathBuf,
    /// The git common directory (shared by worktrees)
    pub git_common_dir: PathBuf,
}

impl RepoPaths {
    /// Returns the `<git common dir>/trail` data directory.
    pub fn data_dir(&self) -> PathBuf {
        self.git_common_dir.join("trail")
    }

    /// Discover the git repository starting from `start_dir` and walking upwards.
    pub fn discover(start_dir: &Path) -> Option<Self> {
        // Fast path: if start_dir already contains .git directory
        let dot_git = start_dir.join(".git");
        if dot_git.is_dir() {
            let work_tree = start_dir.canonicalize().unwrap_or_else(|_| start_dir.to_path_buf());
            let git_dir = work_tree.join(".git");
            let git_common_dir = git_dir.clone();
            return Some(Self { work_tree, git_dir, git_common_dir });
        }

        let canonical_start = start_dir.canonicalize().ok()?;
        let mut curr = canonical_start.as_path();

        loop {
            let dot_git = curr.join(".git");
            if dot_git.is_dir() {
                let git_dir = dot_git.canonicalize().ok()?;
                let git_common_dir = git_dir.clone();
                return Some(Self { work_tree: curr.to_path_buf(), git_dir, git_common_dir });
            } else if dot_git.is_file() {
                // Git worktree or submodule where .git is a file containing "gitdir: <path>"
                if let Ok(content) = fs::read_to_string(&dot_git) {
                    for line in content.lines() {
                        let line = line.trim();
                        if let Some(target) = line.strip_prefix("gitdir:") {
                            let target = target.trim();
                            let target_path = PathBuf::from(target);
                            let git_dir = if target_path.is_absolute() { target_path } else { curr.join(target_path) };
                            let git_dir = git_dir.canonicalize().ok()?;

                            // Check if git_dir contains a "commondir" file (worktree)
                            let commondir_file = git_dir.join("commondir");
                            let git_common_dir = if commondir_file.is_file() {
                                if let Ok(common_content) = fs::read_to_string(&commondir_file) {
                                    let common_target = common_content.trim();
                                    let common_path = PathBuf::from(common_target);
                                    if common_path.is_absolute() {
                                        common_path.canonicalize().unwrap_or_else(|_| git_dir.clone())
                                    } else {
                                        git_dir.join(common_path).canonicalize().unwrap_or_else(|_| git_dir.clone())
                                    }
                                } else {
                                    git_dir.clone()
                                }
                            } else {
                                git_dir.clone()
                            };

                            return Some(Self { work_tree: curr.to_path_buf(), git_dir, git_common_dir });
                        }
                    }
                }
            }

            match curr.parent() {
                Some(parent) => curr = parent,
                None => break,
            }
        }

        None
    }
}

/// Walk all files in `root` respecting .gitignore, .trailignore, and excluding .git/.
pub fn walk_baseline_files(root: &Path) -> Vec<PathBuf> {
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false); // don't skip dotfiles by default (.gitignore, .env etc.)
    builder.git_ignore(true);
    builder.git_global(true);
    builder.git_exclude(true);
    builder.add_custom_ignore_filename(".trailignore");
    // another repository inside (a submodule, a nested clone) has its own history
    let top = root.to_path_buf();
    builder.filter_entry(move |e| e.path() == top || !e.file_type().is_some_and(|t| t.is_dir()) || !e.path().join(".git").exists());

    let mut files = Vec::new();
    for entry in builder.build().flatten() {
        if entry.file_type().is_some_and(|ft| ft.is_file() || ft.is_symlink()) {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                // Always ignore files in .git/
                if rel.starts_with(".git") || tracewise_platform::paths::is_os_junk(rel) {
                    continue;
                }
                files.push(rel.to_path_buf());
            }
        }
    }
    files.sort();
    files
}

/// Count baseline files in `root` respecting .gitignore, .trailignore, and excluding .git/.
pub fn count_baseline_files(root: &Path) -> usize {
    walk_baseline_files(root).len()
}

/// Reads current git branch and HEAD commit hash.
pub fn get_git_info(repo: &RepoPaths) -> (String, String) {
    let head_file = repo.git_dir.join("HEAD");
    let content = fs::read_to_string(&head_file).unwrap_or_default();
    let trimmed = content.trim();

    if let Some(ref_path) = trimmed.strip_prefix("ref:") {
        let ref_path = ref_path.trim();
        let branch = ref_path.strip_prefix("refs/heads/").unwrap_or(ref_path).to_string();

        // Look for commit hash in refs/heads/<branch>
        let direct_ref = repo.git_dir.join(ref_path);
        let common_ref = repo.git_common_dir.join(ref_path);

        let head = if let Ok(sha) = fs::read_to_string(direct_ref).or_else(|_| fs::read_to_string(common_ref)) {
            sha.trim().to_string()
        } else {
            // Check packed-refs in git_dir or git_common_dir
            read_packed_ref(&repo.git_dir.join("packed-refs"), ref_path)
                .or_else(|| read_packed_ref(&repo.git_common_dir.join("packed-refs"), ref_path))
                .unwrap_or_default()
        };

        (branch, head)
    } else {
        ("HEAD".to_string(), trimmed.to_string())
    }
}

fn read_packed_ref(packed_file: &Path, ref_path: &str) -> Option<String> {
    let content = fs::read_to_string(packed_file).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == ref_path {
            return Some(parts[0].to_string());
        }
    }
    None
}

#[cfg(test)]
#[path = "test_lib.rs"]
mod tests;
