//! `tracewise diff --since` shows the difference from the state at that time.
mod common;
use common::Repo;

#[test]
fn diff_since_compares_with_the_state_at_that_time() {
    let repo = Repo::uninitialised("");
    repo.write("a.txt", "one\n");
    repo.write("b.txt", "bee\n");
    assert!(repo.me(&["init"]).ok());
    repo.edit_as("human:me", "a.txt", "one\ntwo\n");
    repo.edit_as("human:me", "b.txt", "bee\nbuzz\n");
    let newest = repo.rows(&[])[0]["time"]["start"].as_str().unwrap().to_string();
    let cutoff = (chrono::DateTime::parse_from_rfc3339(&newest).unwrap() + chrono::Duration::milliseconds(1))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    repo.edit_as("human:me", "a.txt", "one\ntwo\nthree\n");

    let a = repo.me(&["diff", "--since", &cutoff, "a.txt"]).out;
    assert!(a.contains("+three") && !a.contains("+two"), "only the edit after the cutoff: {a}");
    let b = repo.me(&["diff", "--since", &cutoff, "b.txt"]).out;
    assert!(!b.contains('+'), "b.txt did not change since the cutoff, so nothing to show: {b}");
    let all = repo.me(&["diff", "--since", &cutoff]).out;
    assert!(all.contains("a.txt") && !all.contains("b.txt"), "only files that changed since the cutoff: {all}");

    assert_eq!(repo.me(&["diff", "--since", "banana", "a.txt"]).code, 2);
    assert_eq!(repo.me(&["diff"]).code, 2);
}
