use super::*;
use std::fs;
use tempfile::tempdir;
use tracewise_shim::is_tracewise_bin_name;

#[test]
fn test_is_tracewise_bin_name() {
    assert!(is_tracewise_bin_name("tracewise"));
    assert!(is_tracewise_bin_name("tracewise-git"));
    assert!(is_tracewise_bin_name("tracewise-shim"));
    assert!(!is_tracewise_bin_name("sed"));
    assert!(!is_tracewise_bin_name("awk"));
}

#[test]
fn test_find_real_tool_override() {
    let dir = tempdir().unwrap();
    let fake = dir.path().join("fake_sed");
    fs::write(&fake, b"#!/bin/sh\n").unwrap();

    unsafe {
        std::env::set_var("TRACEWISE_REAL_SED", fake.to_str().unwrap());
    }
    let found = find_real_tool("sed", None);
    assert_eq!(found, fake);
    unsafe {
        std::env::remove_var("TRACEWISE_REAL_SED");
    }
}

#[test]
fn test_find_real_tool_skips_tracewise_binaries() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let shim_dir = dir.path().join("shims");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&shim_dir).unwrap();

    let tracewise_bin = bin_dir.join("tracewise-shim");
    fs::write(&tracewise_bin, b"binary").unwrap();

    let fake_shim = shim_dir.join("sed");
    tracewise_platform::shims::create_shim_link(&tracewise_bin, &fake_shim).unwrap();

    let path_str = format!("{}:/usr/bin", shim_dir.display());
    let found = find_real_tool_inner("sed", Some(std::ffi::OsStr::new(&path_str)), Some(&shim_dir));
    assert_ne!(found, fake_shim);
    assert_eq!(found, PathBuf::from("/usr/bin/sed"));
}

#[test]
fn test_find_tracewise_bin_fallback_order() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let sibling_tracewise = bin_dir.join("tracewise");
    fs::write(&sibling_tracewise, b"binary").unwrap();

    let self_exe = bin_dir.join("tracewise-shim");
    fs::write(&self_exe, b"shim").unwrap();

    let found = find_tracewise_bin_inner(Some(&self_exe), None, None);
    assert_eq!(found, Some(sibling_tracewise));
}
