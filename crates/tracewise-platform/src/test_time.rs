use super::*;

#[test]
fn test_local_hhmm_format() {
    let s = local_hhmm(1700000000);
    assert_eq!(s.len(), 5);
    assert!(s.contains(':'));
}
