use super::*;

#[test]
fn test_is_read_only_git() {
    assert!(is_read_only_git(&["rev-parse", "HEAD"]));
    assert!(is_read_only_git(&["status", "--porcelain"]));
    assert!(is_read_only_git(&["log", "--oneline", "-1"]));
    assert!(is_read_only_git(&["diff"]));
    assert!(is_read_only_git(&["show", "HEAD"]));
    assert!(is_read_only_git(&["blame", "file.txt"]));
    assert!(is_read_only_git(&["--version"]));
    assert!(is_read_only_git(&["--help"]));
    assert!(is_read_only_git::<&str>(&[]));
    assert!(is_read_only_git(&["config", "--get", "user.name"]));
    assert!(is_read_only_git(&["remote", "-v"]));
    assert!(is_read_only_git(&["branch"]));
    assert!(is_read_only_git(&["branch", "-a"]));
    assert!(is_read_only_git(&["for-each-ref"]));
    assert!(is_read_only_git(&["stash", "list"]));
    assert!(is_read_only_git(&["stash", "show"]));
    assert!(is_read_only_git(&["worktree", "list"]));

    // Mutating commands
    assert!(!is_read_only_git(&["checkout", "main"]));
    assert!(!is_read_only_git(&["switch", "main"]));
    assert!(!is_read_only_git(&["reset", "--hard"]));
    assert!(!is_read_only_git(&["restore", "file.txt"]));
    assert!(!is_read_only_git(&["commit", "-m", "hello"]));
    assert!(!is_read_only_git(&["merge", "feature"]));
    assert!(!is_read_only_git(&["rebase", "main"]));
    assert!(!is_read_only_git(&["pull"]));
    assert!(!is_read_only_git(&["cherry-pick", "HEAD^"]));
    assert!(!is_read_only_git(&["revert", "HEAD"]));
    assert!(!is_read_only_git(&["am", "patch"]));
    assert!(!is_read_only_git(&["clean", "-fd"]));
    assert!(!is_read_only_git(&["apply", "patch.diff"]));
    assert!(!is_read_only_git(&["stash"]));
    assert!(!is_read_only_git(&["stash", "pop"]));
    assert!(!is_read_only_git(&["stash", "apply"]));
    assert!(!is_read_only_git(&["stash", "push"]));
    assert!(!is_read_only_git(&["clone", "https://example.com/repo.git"]));
    assert!(!is_read_only_git(&["worktree", "add", "new-worktree"]));

    // With global flags
    assert!(is_read_only_git(&["-C", "dir", "status"]));
    assert!(is_read_only_git(&["--git-dir=foo", "rev-parse", "HEAD"]));
    assert!(!is_read_only_git(&["-C", "dir", "checkout", "main"]));
}
