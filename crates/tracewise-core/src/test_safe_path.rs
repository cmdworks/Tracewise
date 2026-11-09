use super::*;

#[test]
fn plain_relative_paths_are_accepted_and_normalised() {
    let dir = tempfile::tempdir().unwrap();
    let (rel, full) = resolve_inside(dir.path(), "./src//a.txt").unwrap();
    assert_eq!(rel, "src/a.txt");
    assert_eq!(full, dir.path().join("src/a.txt"));
}

#[test]
fn paths_that_leave_the_repository_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    for bad in ["../x", "a/../../x", "/etc/passwd", "", ".", ".git/config", "a/.git/b", "a\0b"] {
        assert!(resolve_inside(dir.path(), bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_symlink_in_the_path_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    if tracewise_platform::private::symlink(outside.path(), dir.path().join("link")).is_err() {
        return; // no symlinks on this platform
    }
    tracewise_platform::private::symlink(outside.path().join("f"), dir.path().join("file_link")).unwrap();
    assert!(resolve_inside(dir.path(), "link/evil.txt").is_err());
    assert!(resolve_inside(dir.path(), "file_link").is_err());
}
