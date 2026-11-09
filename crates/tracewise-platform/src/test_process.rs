use super::*;

#[test]
fn test_current_process_chain() {
    let me = std::process::id();
    let chain = process_chain(me);
    assert!(!chain.is_empty());
    assert_eq!(chain[0].pid, me);
}

#[test]
fn the_sed_fast_path_only_takes_arguments_that_cannot_write() {
    let ok = |a: &[&str]| super::sed_args_surely_read_only(a.iter().map(|s| s.as_bytes()));
    assert!(ok(&["-n", "s/a/b/p", "f.txt"]));
    assert!(ok(&["-E", "-e", "s/a+/b/", "f.txt"]));
    for writes in [
        &["-i", "s/a/b/", "f"][..],
        &["-Ei", "s/a/b/", "f"],
        &["-ni", "p", "f"],
        &["-i.bak", "s/a/b/", "f"],
        &["--in-place", "s/a/b/", "f"],
        &["-f", "x.sed", "f"],
        &["--file=x.sed", "f"],
        &["-n", "s/a/b/w out", "f"],
        &["--tracewise-why=x", "s/a/b/", "f"],
    ] {
        assert!(!ok(writes), "{writes:?}");
    }
}
