use super::*;

#[test]
fn registering_lists_once_and_forgets_what_is_gone() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let make = |n: &str| {
        let wt = tmp.path().join(n);
        let dd = wt.join(".git/trail");
        fs::create_dir_all(&dd).unwrap();
        fs::write(dd.join("trail.db"), "").unwrap();
        (wt, dd)
    };
    let (a, ad) = make("a");
    let (b, bd) = make("b");
    assert!(register_in(&home, &a, &ad));
    assert!(!register_in(&home, &a, &ad), "twice is once");
    assert!(register_in(&home, &b, &bd));
    assert_eq!(list_in(&home).len(), 2);
    fs::remove_dir_all(&a).unwrap();
    assert_eq!(list_in(&home).len(), 1, "a deleted repository drops out");
    let (c, cd) = make("c");
    assert!(register_in(&home, &c, &cd));
    assert_eq!(fs::read_to_string(file_in(&home)).unwrap().lines().count(), 2, "and is dropped from the file when it changes");
}
