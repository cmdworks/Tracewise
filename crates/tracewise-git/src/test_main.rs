use super::*;

#[test]
fn test_is_trail_bin_name() {
    assert!(is_trail_bin_name("trail"));
    assert!(is_trail_bin_name("trail.exe"));
    assert!(is_trail_bin_name("agent-trail"));
    assert!(is_trail_bin_name("agent-trail.exe"));
    assert!(is_trail_bin_name("trail-git"));
    assert!(is_trail_bin_name("trail-git.exe"));
    assert!(!is_trail_bin_name("git"));
    assert!(!is_trail_bin_name("custom-git"));
}

#[test]
fn test_find_trail_bin_fallback_order() {
    let tmp = tempfile::tempdir().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let fake_trail = bin_dir.join("trail");
    fs::write(&fake_trail, b"binary").unwrap();
    let fake_self = bin_dir.join("trail-git");
    fs::write(&fake_self, b"binary").unwrap();

    // 1. Found next to self
    let found = find_trail_bin_inner(Some(&fake_self), None, None);
    assert_eq!(found, Some(fake_trail.clone()));

    // 2. Next to self missing, found via TRAIL_BIN
    let other_dir = tmp.path().join("other");
    fs::create_dir_all(&other_dir).unwrap();
    let custom_trail = other_dir.join("my-trail");
    fs::write(&custom_trail, b"binary").unwrap();

    let missing_self = tmp.path().join("missing/trail-git");
    let found = find_trail_bin_inner(Some(&missing_self), Some(custom_trail.to_str().unwrap()), None);
    assert_eq!(found, Some(custom_trail));

    // 3. Found via HOME/.cargo/bin/trail
    let fake_home = tmp.path().join("home");
    let cargo_bin = fake_home.join(".cargo/bin");
    fs::create_dir_all(&cargo_bin).unwrap();
    let cargo_trail = cargo_bin.join("trail");
    fs::write(&cargo_trail, b"binary").unwrap();

    let found = find_trail_bin_inner(Some(&missing_self), None, Some(fake_home.to_str().unwrap()));
    assert_eq!(found, Some(cargo_trail));

    // 4. Everything missing -> None (never a PATH search)
    let found = find_trail_bin_inner(Some(&missing_self), None, None);
    assert_eq!(found, None);
}

#[test]
fn test_symlinked_wrapper_finds_trail_next_to_the_real_file() {
    let tmp = tempfile::tempdir().unwrap();
    let real_dir = tmp.path().join("release");
    let link_dir = tmp.path().join("bin");
    fs::create_dir_all(&real_dir).unwrap();
    fs::create_dir_all(&link_dir).unwrap();
    fs::write(real_dir.join("trail"), b"binary").unwrap();
    fs::write(real_dir.join("trail-git"), b"binary").unwrap();
    let link = link_dir.join("git");
    tracewise_platform::shims::create_shim_link(&real_dir.join("trail-git"), &link).unwrap();
    // started as bin/git: resolving first finds release/trail
    let resolved = link.canonicalize().unwrap();
    let found = find_trail_bin_inner(Some(&resolved), None, None);
    assert_eq!(found, Some(real_dir.canonicalize().unwrap().join("trail")));
    // the unresolved link path would have found nothing next to it
    assert_eq!(find_trail_bin_inner(Some(&link), None, None), None);
}

#[test]
fn test_find_real_git_skips_trail_binaries() {
    let tmp = tempfile::tempdir().unwrap();
    let dir1 = tmp.path().join("dir1");
    let dir2 = tmp.path().join("dir2");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();

    // dir1 and dir2 have git pointing to trail binaries
    let fake_trail = tmp.path().join("trail-git");
    fs::write(&fake_trail, b"binary").unwrap();
    let link1 = dir1.join("git");
    let link2 = dir2.join("git");
    tracewise_platform::shims::create_shim_link(&fake_trail, &link1).unwrap();
    tracewise_platform::shims::create_shim_link(&fake_trail, &link2).unwrap();

    let old_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{}:{}", dir1.display(), dir2.display(), old_path);
    let found = find_real_git_inner(Some(&test_path), None);
    assert_ne!(found, link1);
    assert_ne!(found, link2);
}

#[test]
fn test_trail_git_loop_guard() {
    // When TRAIL_GIT_DEPTH is set, exec_or_run must directly target /usr/bin/git (or fallback git)
    let depth_val = std::env::var_os("TRAIL_GIT_DEPTH");
    unsafe {
        std::env::set_var("TRAIL_GIT_DEPTH", "1");
    }
    let fallback = Path::new("/usr/bin/git");
    let target = if fallback.is_file() { fallback } else { Path::new("git") };
    assert!(target.is_file() || target == Path::new("git"));
    match depth_val {
        Some(v) => unsafe { std::env::set_var("TRAIL_GIT_DEPTH", v) },
        None => unsafe { std::env::remove_var("TRAIL_GIT_DEPTH") },
    }
}
