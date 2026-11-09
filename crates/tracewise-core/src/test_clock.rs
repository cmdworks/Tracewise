use super::*;

#[test]
fn without_tracewise_now_the_clock_is_real() {
    let before = Utc::now();
    let t = now();
    assert!(t >= before && t <= Utc::now() + chrono::Duration::seconds(1));
    assert!(now_text().ends_with('Z') && now_text().len() == 24);
}
