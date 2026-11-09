use super::*;

#[test]
fn shim_link_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("real_bin");
    fs::write(&target, b"binary_content").unwrap();

    let link = temp.path().join("link_bin");
    create_shim_link(&target, &link).unwrap();

    assert!(link.exists());
    #[cfg(unix)]
    assert!(link.is_symlink());
}

#[test]
fn shim_link_copy_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("real_bin");
    fs::write(&target, b"fallback_payload").unwrap();

    let copy_dest = temp.path().join("copied_bin");
    create_shim_copy(&target, &copy_dest).unwrap();

    assert!(copy_dest.exists());
    assert_eq!(fs::read(&copy_dest).unwrap(), b"fallback_payload");
}

#[test]
fn shim_link_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("real_bin");
    fs::write(&target, b"idempotent_test").unwrap();

    let link = temp.path().join("link_bin");
    create_shim_link(&target, &link).unwrap();
    // Repeating should succeed and replace without error
    create_shim_link(&target, &link).unwrap();

    assert!(link.exists());
}

#[test]
fn shim_link_set_executable() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("script.sh");
    fs::write(&target, b"#!/bin/sh\necho hi\n").unwrap();
    set_executable(&target).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
    }
}
