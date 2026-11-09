use super::*;

#[test]
fn merge_keeps_other_keys_and_unmerge_removes_only_ours() {
    let mut cfg = json!({"other": 1, "hooks": {"mine": ["keep"]}});
    let ours = json!({"hooks": {"mine": ["tracewise"], "before": "tracewise hook"}});
    merge(&mut cfg, &ours);
    merge(&mut cfg, &ours);
    assert_eq!(cfg, json!({"other": 1, "hooks": {"mine": ["keep", "tracewise"], "before": "tracewise hook"}}));
    assert!(!unmerge(&mut cfg, &ours));
    assert_eq!(cfg, json!({"other": 1, "hooks": {"mine": ["keep"]}}));
}

#[test]
fn a_file_made_only_of_our_entries_goes_away() {
    let ours = json!({"hooks": {"before": "tracewise hook"}});
    let mut cfg = json!({});
    merge(&mut cfg, &ours);
    assert!(unmerge(&mut cfg, &ours));
}
