use super::*;

#[test]
fn paths_private_var_equivalence() {
    let p1 = Path::new("/private/var/folders/xyz/repo");
    let p2 = Path::new("/var/folders/xyz/repo");
    assert_eq!(normalize_virtual_path(p1), p2);
    assert!(paths_equal(p1, p2));
}

#[test]
fn paths_private_tmp_equivalence() {
    let p1 = Path::new("/private/tmp/session-123.log");
    let p2 = Path::new("/tmp/session-123.log");
    assert_eq!(normalize_virtual_path(p1), p2);
    assert!(paths_equal(p1, p2));
}

#[test]
fn paths_windows_separators_normalized() {
    let win_path = r"C:\Users\test\Developer\project\src\main.rs";
    let expected = "C:/Users/test/Developer/project/src/main.rs";
    assert_eq!(normalize_windows_separators(win_path), expected);
}

#[test]
fn paths_windows_extended_prefix_stripped() {
    let win_ext = r"\\?\C:\Users\test\project\README.md";
    let expected = "C:/Users/test/project/README.md";
    assert_eq!(normalize_windows_separators(win_ext), expected);
}

#[test]
fn os_junk_is_recognised_anywhere_in_the_tree() {
    for junk in [".DS_Store", "client/.DS_Store", "a/._file.txt", "Thumbs.db", ".Trashes/501/x", "d/.Spotlight-V100/Store"] {
        assert!(is_os_junk(Path::new(junk)), "{junk}");
    }
    for work in ["src/main.rs", ".gitignore", "._", "notes.DS_Store.txt", "a/b/c.txt"] {
        assert!(!is_os_junk(Path::new(work)), "{work}");
    }
}
