use super::*;
use std::fs;
use std::path::PathBuf;
use tracewise_shim::detect_sed_targets;

#[test]
fn test_strip_tracewise_why() {
    let args = vec!["-i".to_string(), "s/a/b/".to_string(), "--tracewise-why=fix typo".to_string(), "file.txt".to_string()];
    let (clean, why) = strip_tracewise_why(&args);
    assert_eq!(clean, vec!["-i", "s/a/b/", "file.txt"]);
    assert_eq!(why, Some("fix typo".to_string()));

    let args2 = vec!["-i".to_string(), "file.txt".to_string()];
    let (clean2, why2) = strip_tracewise_why(&args2);
    assert_eq!(clean2, vec!["-i", "file.txt"]);
    assert_eq!(why2, None);
}

#[test]
fn test_is_read_only_sed() {
    assert!(is_read_only("sed", &["s/a/b/".to_string(), "file.txt".to_string()]));
    assert!(is_read_only("sed", &["-n".to_string(), "/p/p".to_string(), "file.txt".to_string()]));
    assert!(is_read_only("sed", &["-e".to_string(), "s/a/b/".to_string(), "file.txt".to_string()]));
    assert!(!is_read_only("sed", &["-i".to_string(), "s/a/b/".to_string(), "file.txt".to_string()]));
    assert!(!is_read_only("sed", &["-i''".to_string(), "s/a/b/".to_string(), "file.txt".to_string()]));
    assert!(!is_read_only("sed", &["-i.bak".to_string(), "s/a/b/".to_string(), "file.txt".to_string()]));
    assert!(!is_read_only("sed", &["--in-place".to_string(), "s/a/b/".to_string(), "file.txt".to_string()]));
    assert!(!is_read_only("sed", &["w out.txt".to_string(), "file.txt".to_string()]));
}

#[test]
fn test_is_read_only_perl_awk_tee() {
    assert!(is_read_only("perl", &["-e".to_string(), "print 1".to_string()]));
    assert!(!is_read_only("perl", &["-pi".to_string(), "-e".to_string(), "s/a/b/".to_string()]));

    assert!(is_read_only("awk", &["{print}".to_string(), "file.txt".to_string()]));
    assert!(!is_read_only("awk", &["-i".to_string(), "inplace".to_string(), "{print}".to_string()]));

    assert!(is_read_only("tee", &[]));
    assert!(is_read_only("tee", &["-a".to_string()]));
    assert!(!is_read_only("tee", &["out.txt".to_string()]));

    assert!(!is_read_only("mv", &["a".to_string(), "b".to_string()]));
    assert!(!is_read_only("cp", &["a".to_string(), "b".to_string()]));
    assert!(!is_read_only("rm", &["a".to_string()]));
}

#[test]
fn test_fast_read_only_shim_guard() {
    std::env::set_var("TRACEWISE_SHIM_DEPTH", "1");
    assert!(!try_fast_read_only_shim());
    std::env::remove_var("TRACEWISE_SHIM_DEPTH");
}

#[test]
fn test_find_real_tool_override() {
    std::env::set_var("TRACEWISE_REAL_SED", "/bin/echo");
    let tool = find_real_tool("sed", None);
    assert_eq!(tool, PathBuf::from("/bin/echo"));
    std::env::remove_var("TRACEWISE_REAL_SED");
}

#[test]
fn test_find_real_tool_skips_shims_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let shims_dir = tmp.path().join("shims");
    fs::create_dir_all(&shims_dir).unwrap();
    let mock_shim = shims_dir.join("sed");
    let fake_bin = tmp.path().join("tracewise");
    fs::write(&fake_bin, b"fake tracewise").unwrap();
    tracewise_platform::shims::create_shim_link(&fake_bin, &mock_shim).unwrap();

    let tool = find_real_tool("sed", Some(&shims_dir));
    assert_ne!(tool, mock_shim);
    assert!(tool.is_file());
}

#[test]
fn test_detect_sed_targets_gnu_and_bsd() {
    // BSD form: sed -i '' 's/a/b/' file.txt
    let args = vec!["-i".to_string(), "".to_string(), "s/a/b/".to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::Paths(vec![PathBuf::from("file.txt")]));

    // BSD form with backup: sed -i.bak 's/a/b/' file.txt
    let args = vec!["-i.bak".to_string(), "s/a/b/".to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::Paths(vec![PathBuf::from("file.txt"), PathBuf::from("file.txt.bak")]));

    // GNU form: sed -i 's/a/b/' file.txt
    let args = vec!["-i".to_string(), "s/a/b/".to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::Paths(vec![PathBuf::from("file.txt")]));

    // GNU in-place with backup: sed --in-place=.orig 's/a/b/' file.txt
    let args = vec!["--in-place=.orig".to_string(), "s/a/b/".to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::Paths(vec![PathBuf::from("file.txt"), PathBuf::from("file.txt.orig")]));

    // Bundled flags: sed -Ei.bak 's/a/b/' file.txt
    let args = vec!["-Ei.bak".to_string(), "s/a/b/".to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::Paths(vec![PathBuf::from("file.txt"), PathBuf::from("file.txt.bak")]));

    // Script w command: sed -n 'w out.txt' file.txt
    let args = vec!["-n".to_string(), "w out.txt".to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::Paths(vec![PathBuf::from("out.txt")]));

    // Script s///w command: sed -e 's/a/b/w out2.txt' file.txt
    let args = vec!["-e".to_string(), "s/a/b/w out2.txt".to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::Paths(vec![PathBuf::from("out2.txt")]));

    // Script -f file: sed -n -f prog.sed file.txt
    let tmp = tempfile::tempdir().unwrap();
    let prog = tmp.path().join("prog.sed");
    fs::write(&prog, b"s/y/z/\nw out3.txt\n").unwrap();
    let args = vec!["-n".to_string(), "-f".to_string(), prog.to_str().unwrap().to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::Paths(vec![PathBuf::from("out3.txt")]));

    // Read-only sed
    let args = vec!["s/a/b/".to_string(), "file.txt".to_string()];
    assert_eq!(detect_sed_targets(&args), WriteTargets::None);
}

#[test]
fn test_detect_write_targets_all_tools() {
    // Perl: read-only vs -i vs -pi
    assert_eq!(detect_write_targets("perl", &["-e".to_string(), "print 1".to_string()]), WriteTargets::None);
    assert_eq!(
        detect_write_targets("perl", &["-pi".to_string(), "-e".to_string(), "s/a/b/".to_string(), "file.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("file.txt")])
    );
    assert_eq!(
        detect_write_targets("perl", &["-i.bak".to_string(), "-e".to_string(), "s/a/b/".to_string(), "file.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("file.txt"), PathBuf::from("file.txt.bak")])
    );

    // Awk: read-only vs -i inplace
    assert_eq!(detect_write_targets("awk", &["{print}".to_string(), "file.txt".to_string()]), WriteTargets::None);
    assert_eq!(
        detect_write_targets("awk", &["-i".to_string(), "inplace".to_string(), "{print}".to_string(), "file.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("file.txt")])
    );

    // Tee: read-only vs with files
    assert_eq!(detect_write_targets("tee", &[]), WriteTargets::None);
    assert_eq!(
        detect_write_targets("tee", &["-a".to_string(), "out.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("out.txt")])
    );

    // Mv, cp, ln
    assert_eq!(
        detect_write_targets("mv", &["src.txt".to_string(), "dst.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("dst.txt"), PathBuf::from("src.txt")])
    );
    assert_eq!(
        detect_write_targets("cp", &["src.txt".to_string(), "dst.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("dst.txt")])
    );
    assert_eq!(
        detect_write_targets("ln", &["-s".to_string(), "src.txt".to_string(), "dst.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("dst.txt")])
    );

    // Rm, truncate, touch
    assert_eq!(
        detect_write_targets("rm", &["-f".to_string(), "file.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("file.txt")])
    );
    assert_eq!(
        detect_write_targets("truncate", &["-s".to_string(), "0".to_string(), "file.txt".to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("file.txt")])
    );
    assert_eq!(detect_write_targets("touch", &["file.txt".to_string()]), WriteTargets::Paths(vec![PathBuf::from("file.txt")]));

    // Patch: parsing patch text
    let tmp = tempfile::tempdir().unwrap();
    let patch = tmp.path().join("file.patch");
    let patch_content = b"--- a/old.txt\n+++ b/target.txt\n@@ -1 +1 @@\n-old\n+new\n";
    fs::write(&patch, patch_content).unwrap();
    assert_eq!(
        detect_write_targets("patch", &["-p1".to_string(), "-i".to_string(), patch.to_str().unwrap().to_string()]),
        WriteTargets::Paths(vec![PathBuf::from("target.txt")])
    );

    // Unparsable
    assert_eq!(detect_write_targets("mv", &[]), WriteTargets::AnyInCwd);
    assert_eq!(detect_write_targets("patch", &[]), WriteTargets::AnyInCwd);
}

#[test]
fn test_log_shim_error() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    std::env::set_var("TRACEWISE_HOME", &home);

    log_shim_error("database corrupt test error");
    let log_file = home.join("shim.log");
    assert!(log_file.is_file());
    let content = fs::read_to_string(&log_file).unwrap();
    assert!(content.contains("database corrupt test error"));

    std::env::remove_var("TRACEWISE_HOME");
}
