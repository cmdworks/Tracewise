use super::*;
use std::fs;

#[test]
fn gitignore_rules_apply_to_single_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".git/info")).unwrap();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join(".gitignore"), "/target\n*.log\n!keep.log\nnode_modules\n").unwrap();
    fs::write(root.join("sub/.gitignore"), "*.tmp\n").unwrap();
    fs::write(root.join(".git/info/exclude"), "secret.txt\n").unwrap();
    fs::write(root.join(".tracewiseignore"), "generated/\n").unwrap();
    let mut r = IgnoreRules::load(root);
    for ignored in [
        "target/debug/tracewise",
        "a.log",
        "deep/b.log",
        "web/node_modules/x/index.js",
        "sub/x.tmp",
        "secret.txt",
        "generated/out.rs",
        ".DS_Store",
    ] {
        assert!(r.is_ignored(Path::new(ignored)), "{ignored} should be ignored");
    }
    for kept in ["src/main.rs", "keep.log", "x.tmp", "targets/a.txt", "sub/y.txt", ".gitignore"] {
        assert!(!r.is_ignored(Path::new(kept)), "{kept} should be recorded");
    }
}
