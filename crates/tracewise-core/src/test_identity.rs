use super::*;

fn repo(dir: &Path) -> PathBuf {
    let wt = dir.to_path_buf();
    fs::create_dir_all(wt.join(".git").join("tracewise")).unwrap();
    wt
}

#[test]
fn a_copy_is_told_from_a_move() {
    let tmp = tempfile::tempdir().unwrap();
    let a = repo(&tmp.path().join("a"));
    let data = a.join(".git").join("tracewise");
    assert_eq!(check_copy(&data, &a), CopyState::Original);
    let id = repo_id(&data, &a);

    // copy: the original is still there with the same id
    fs::create_dir_all(tmp.path().join("b").join(".git")).unwrap();
    let b_data = tmp.path().join("b").join(".git").join("tracewise");
    fs::create_dir_all(&b_data).unwrap();
    for f in ["id", "home"] {
        fs::copy(data.join(f), b_data.join(f)).unwrap();
    }
    let b = tmp.path().join("b");
    match check_copy(&b_data, &b) {
        CopyState::Copy { original } => assert_eq!(original, a.canonicalize().unwrap()),
        other => panic!("{other:?}"),
    }

    // adopting gives it its own id and device and ends the warning
    let (old, new, device) = adopt_copy(&b_data, &b).unwrap();
    assert_eq!(old, id);
    assert_ne!(new, id);
    assert!(device.contains("-copy-"));
    assert_eq!(check_copy(&b_data, &b), CopyState::Original);
    assert_eq!(device_for(&b_data), device);
    assert_eq!(check_copy(&data, &a), CopyState::Original);

    // move: the old path no longer holds this id
    fs::remove_dir_all(&a).unwrap();
    let c = tmp.path().join("c");
    fs::rename(&b, &c).unwrap();
    let c_data = c.join(".git").join("tracewise");
    assert_eq!(check_copy(&c_data, &c), CopyState::Moved);
    assert_eq!(check_copy(&c_data, &c), CopyState::Original);
}
