use super::*;
use std::fs;

#[test]
fn test_device_id_env() {
    std::env::set_var("TRAIL_DEVICE", "dev-custom");
    assert_eq!(get_device_id(), "dev-custom");
}

#[test]
fn test_record_baseline() {
    let temp = tempfile::tempdir().unwrap();
    let work_tree = temp.path().join("repo");
    let data_dir = work_tree.join(".git/trail");
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(work_tree.join("README.md"), "# Hello\n").unwrap();

    let files = vec![PathBuf::from("README.md")];
    let count = record_baseline(&work_tree, &data_dir, "main", "12345678", &files).unwrap();
    assert_eq!(count, 1);

    let db = Database::open(&data_dir.join("trail.db")).unwrap();
    assert_eq!(db.count_changes().unwrap(), 1);
    let recs = db.list_changes(Some("README.md"), None).unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].actor.kind, "baseline");
}
