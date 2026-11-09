use super::*;

#[test]
fn test_line_tracking() {
    let v1 = "alpha\nbeta\ngamma\n";
    let l1 = update_lines_for_change(None, Some(v1), &[], "c1");
    assert_eq!(l1, vec!["c1", "c1", "c1"]);

    let v2 = "alpha\nBETA\ngamma\n";
    let l2 = update_lines_for_change(Some(v1), Some(v2), &l1, "c2");
    assert_eq!(l2, vec!["c1", "c2", "c1"]);

    let v3 = "alpha\nBETA\ngamma\ndelta\n";
    let l3 = update_lines_for_change(Some(v2), Some(v3), &l2, "c3");
    assert_eq!(l3, vec!["c1", "c2", "c1", "c3"]);
}
