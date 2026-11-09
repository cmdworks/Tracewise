use super::*;

#[test]
fn test_shims_install_and_uninstall() {
    let tmp = tempfile::tempdir().unwrap();
    let shims_dir = tmp.path().join("shims");
    let fake_bin = tmp.path().join("trail");
    fs::write(&fake_bin, b"mock trail binary").unwrap();

    // 1. Install
    let tools = install_shims(&shims_dir, &fake_bin).unwrap();
    assert_eq!(tools.len(), SHIM_TOOLS.len());
    for &t in SHIM_TOOLS {
        assert!(tools.contains(&t.to_string()));
        let p = shims_dir.join(t);
        assert!(p.is_symlink());
        assert_eq!(p.canonicalize().unwrap(), fake_bin.canonicalize().unwrap());
    }

    // Add a non-trail symlink and non-trail regular file
    let foreign_target = tmp.path().join("foreign");
    fs::write(&foreign_target, b"foreign").unwrap();
    let foreign_link = shims_dir.join("foreign_tool");
    tracewise_platform::shims::create_shim_link(&foreign_target, &foreign_link).unwrap();
    let foreign_file = shims_dir.join("foreign_file.txt");
    fs::write(&foreign_file, b"keep me").unwrap();

    // 2. Uninstall
    let removed = uninstall_shims(&shims_dir, Some(&fake_bin));
    assert_eq!(removed, SHIM_TOOLS.len());

    // All trail shims removed
    for &t in SHIM_TOOLS {
        assert!(!shims_dir.join(t).exists());
    }

    // Foreign items kept
    assert!(foreign_link.exists());
    assert!(foreign_file.exists());
}

#[test]
fn test_shims_install_target_resolution() {
    let tmp = tempfile::tempdir().unwrap();
    let bin_dir = tmp.path().join("bin");
    let shims_dir = tmp.path().join("shims");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&shims_dir).unwrap();

    let trail_bin = bin_dir.join("trail");
    fs::write(&trail_bin, b"mock trail binary").unwrap();

    // 1. trail-shim missing -> falls back to trail
    let target1 = trail_bin.parent().map(|p| p.join("trail-shim")).filter(|p| p.is_file()).unwrap_or_else(|| trail_bin.clone());
    assert_eq!(target1, trail_bin);

    // 2. trail-shim present -> selects trail-shim
    let tracewise_shim = bin_dir.join("trail-shim");
    fs::write(&tracewise_shim, b"mock trail-shim binary").unwrap();
    let target2 = trail_bin.parent().map(|p| p.join("trail-shim")).filter(|p| p.is_file()).unwrap_or_else(|| trail_bin.clone());
    assert_eq!(target2, tracewise_shim);
}

#[test]
fn test_shims_install_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let shims_dir = tmp.path().join("shims");
    let fake_bin = tmp.path().join("trail");
    fs::write(&fake_bin, b"mock trail binary").unwrap();

    let tools1 = install_shims(&shims_dir, &fake_bin).unwrap();
    let tools2 = install_shims(&shims_dir, &fake_bin).unwrap();
    assert_eq!(tools1, tools2);
    assert_eq!(tools2.len(), SHIM_TOOLS.len());
    assert_eq!(tools2.len(), 5); // the five content-writing tools

    let touch_link = shims_dir.join("touch");
    tracewise_platform::shims::create_shim_link(&fake_bin, &touch_link).unwrap();
    assert!(touch_link.is_symlink());
    install_shims(&shims_dir, &fake_bin).unwrap();
    assert!(!touch_link.exists());

    // links left by an older Tracewise for the removed tools are unlinked on upgrade
    for old in ["chflags", "xattr"] {
        let link = shims_dir.join(old);
        tracewise_platform::shims::create_shim_link(&fake_bin, &link).unwrap();
        install_shims(&shims_dir, &fake_bin).unwrap();
        assert!(!link.exists(), "{old} link should be removed");
    }
}
