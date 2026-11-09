use super::*;
use std::fs;

#[test]
fn test_discover_standard_repo() {
    let temp = tempfile::tempdir().unwrap();
    let repo_dir = temp.path().join("repo");
    fs::create_dir_all(repo_dir.join(".git")).unwrap();
    fs::create_dir_all(repo_dir.join("src")).unwrap();

    let discovered = RepoPaths::discover(&repo_dir.join("src")).unwrap();
    assert_eq!(discovered.work_tree, repo_dir.canonicalize().unwrap());
    assert_eq!(discovered.git_dir, repo_dir.join(".git").canonicalize().unwrap());
    assert_eq!(discovered.git_common_dir, repo_dir.join(".git").canonicalize().unwrap());
    assert_eq!(discovered.data_dir(), repo_dir.join(".git").canonicalize().unwrap().join("tracewise"));
}

#[test]
fn test_discover_worktree() {
    let temp = tempfile::tempdir().unwrap();
    let main_repo = temp.path().join("main");
    let main_git = main_repo.join(".git");
    let wt_git_dir = main_git.join("worktrees").join("wt1");
    fs::create_dir_all(&wt_git_dir).unwrap();
    fs::write(wt_git_dir.join("commondir"), "../..\n").unwrap();

    let wt_repo = temp.path().join("wt1");
    fs::create_dir_all(&wt_repo).unwrap();
    fs::write(wt_repo.join(".git"), format!("gitdir: {}\n", wt_git_dir.display())).unwrap();

    let discovered = RepoPaths::discover(&wt_repo).unwrap();
    assert_eq!(discovered.work_tree, wt_repo.canonicalize().unwrap());
    assert_eq!(discovered.git_dir, wt_git_dir.canonicalize().unwrap());
    assert_eq!(discovered.git_common_dir, main_git.canonicalize().unwrap());
    assert_eq!(discovered.data_dir(), main_git.canonicalize().unwrap().join("tracewise"));
}

#[test]
fn test_discover_none() {
    let temp = tempfile::tempdir().unwrap();
    assert!(RepoPaths::discover(temp.path()).is_none());
}

#[test]
fn test_baseline_files_counting() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::write(repo.join(".git/config"), "test").unwrap();
    fs::write(repo.join("README.md"), "# Hello").unwrap();
    fs::write(repo.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(repo.join("ignored.txt"), "secret").unwrap();
    fs::write(repo.join(".tracewiseignore"), "skip.txt\n").unwrap();
    fs::write(repo.join("skip.txt"), "skipped").unwrap();

    let files = walk_baseline_files(repo);
    assert_eq!(files, vec![PathBuf::from(".gitignore"), PathBuf::from(".tracewiseignore"), PathBuf::from("README.md")]);
    assert_eq!(count_baseline_files(repo), 3);
}

#[test]
fn test_get_git_info() {
    let temp = tempfile::tempdir().unwrap();
    let repo_dir = temp.path().join("repo");
    let git_dir = repo_dir.join(".git");
    fs::create_dir_all(git_dir.join("refs/heads")).unwrap();
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature\n").unwrap();
    fs::write(git_dir.join("refs/heads/feature"), "abcdef1234567890\n").unwrap();

    let repo = RepoPaths { work_tree: repo_dir.clone(), git_dir: git_dir.clone(), git_common_dir: git_dir };

    let (branch, head) = get_git_info(&repo);
    assert_eq!(branch, "feature");
    assert_eq!(head, "abcdef1234567890");
}
