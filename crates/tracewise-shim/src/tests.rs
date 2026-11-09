use std::fs;
use std::path::PathBuf;

use super::*;

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn copy_operands_split_sources_and_destination() {
    assert_eq!(
        copy_operands("cp", &v(&["-R", "src", "lib", "/out"])),
        Some((vec!["src".to_string(), "lib".to_string()], "/out".to_string()))
    );
    assert_eq!(copy_operands("mv", &v(&["a"])), None);
    assert_eq!(copy_operands("rm", &v(&["a", "b"])), None);
}

#[test]
fn chflags_and_xattr_write_only_when_they_change_something() {
    assert_eq!(detect_write_targets("chflags", &v(&["nouchg", "f.txt"])), WriteTargets::Paths(vec![PathBuf::from("f.txt")]));
    assert!(is_read_only("chflags", &v(&["nouchg"])));
    assert!(is_read_only("xattr", &v(&["-l", "f.txt"])));
    assert_eq!(
        detect_write_targets("xattr", &v(&["-d", "com.apple.quarantine", "f.txt"])),
        WriteTargets::Paths(vec![PathBuf::from("f.txt")])
    );
    assert_eq!(detect_write_targets("xattr", &v(&["-w", "k", "v", "f.txt"])), WriteTargets::Paths(vec![PathBuf::from("f.txt")]));
    assert_eq!(detect_write_targets("xattr", &v(&["-c", "a", "b"])), WriteTargets::Paths(vec![PathBuf::from("a"), PathBuf::from("b")]));
}

#[test]
fn multibyte_sed_delimiters_do_not_panic() {
    assert!(extract_sed_script_w_targets("s€l€L€").is_empty());
    assert!(extract_sed_script_w_targets("sé").is_empty());
    assert!(extract_sed_script_w_targets("y€a€b€").is_empty());
    assert!(extract_sed_script_w_targets("s").is_empty());
    assert!(extract_sed_script_w_targets("").is_empty());
    assert_eq!(extract_sed_script_w_targets("s€a€b€w out.txt"), vec![PathBuf::from("out.txt")]);
}

#[test]
fn the_classifier_never_panics_on_random_input() {
    let alphabet: Vec<char> = "s/\\;wWyie€é -.\n'\"p{}".chars().collect();
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..3000 {
        let len = (next() % 12) as usize;
        let text: String = (0..len).map(|_| alphabet[(next() as usize) % alphabet.len()]).collect();
        let words: Vec<String> = text.split(' ').map(String::from).collect();
        for tool in ["sed", "awk", "gawk", "perl", "tee", "mv", "cp", "rm", "truncate", "patch", "ln", "touch", "other"] {
            let _ = detect_write_targets(tool, &words);
            let _ = detect_write_targets(tool, std::slice::from_ref(&text));
            let _ = is_read_only(tool, &words);
        }
        let _ = extract_sed_script_w_targets(&text);
    }
}

#[test]
fn after_double_dash_every_argument_is_a_file() {
    let paths = |names: &[&str]| WriteTargets::Paths(names.iter().map(PathBuf::from).collect());
    assert_eq!(detect_write_targets("tee", &v(&["--", "out.txt"])), paths(&["out.txt"]));
    assert_eq!(detect_write_targets("tee", &v(&["-a", "--", "out.txt"])), paths(&["out.txt"]));
    assert_eq!(detect_write_targets("tee", &v(&["--", "-dash"])), paths(&["-dash"]));
    assert_eq!(detect_write_targets("tee", &v(&["-a"])), WriteTargets::None);
    assert_eq!(detect_write_targets("tee", &v(&[])), WriteTargets::None);
    assert_eq!(detect_write_targets("touch", &v(&["--", "-x"])), paths(&["-x"]));
    assert_eq!(detect_write_targets("rm", &v(&["-f", "--", "-weird"])), paths(&["-weird"]));
}

#[test]
fn gnu_sed_long_options_are_understood() {
    let paths = |names: &[&str]| WriteTargets::Paths(names.iter().map(PathBuf::from).collect());
    let d = |a: &[&str]| detect_sed_targets(&v(a));
    assert_eq!(d(&["--expression=w out.txt", "f"]), paths(&["out.txt"]));
    assert_eq!(d(&["--expression", "s/a/b/w o.txt", "f"]), paths(&["o.txt"]));
    assert_eq!(d(&["--quiet", "s/a/b/w x", "f"]), paths(&["x"]));
    assert_eq!(d(&["--quiet", "--expression=p", "f"]), WriteTargets::None);
    assert_eq!(d(&["--in-place=.bak", "s/a/b/", "f"]), paths(&["f", "f.bak"]));
    assert_eq!(d(&["--strange-option", "p", "f"]), WriteTargets::AnyInCwd);
    assert_eq!(d(&["--expression"]), WriteTargets::AnyInCwd);
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("s.sed");
    fs::write(&script, "w from_script.txt\n").unwrap();
    let long = format!("--file={}", script.display());
    assert_eq!(d(&[long.as_str(), "f"]), paths(&["from_script.txt"]));
    assert_eq!(d(&["--file", script.to_str().unwrap(), "f"]), paths(&["from_script.txt"]));
}

/// every in-place form writes; none may be classed as read-only.
#[test]
fn in_place_sed_forms_are_never_read_only() {
    let forms: &[&[&str]] = &[
        &["-i", "", "s/a/b/", "f.txt"],
        &["-i.bak", "s/a/b/", "f.txt"],
        &["-i", ".bak", "s/a/b/", "f.txt"],
        &["--in-place", "s/a/b/", "f.txt"],
        &["--in-place=.bak", "s/a/b/", "f.txt"],
        &["-Ei", "s/a/b/", "f.txt"],
        &["-ni", "p", "f.txt"],
        &["-n", "-i", "p", "f.txt"],
        &["-e", "s/a/b/", "-i", "f.txt"],
        &["-i", "-e", "s/a/b/", "--", "f.txt"],
        &["-E", "-i", "", "-e", "s/a/b/", "f.txt", "g.txt"],
        &["s/a/b/w out.txt", "f.txt"],
        &["-f", "script.sed", "f.txt"],
    ];
    for args in forms {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        assert_ne!(crate::detect_write_targets("sed", &owned), crate::WriteTargets::None, "sed {args:?} writes");
    }
    let read: Vec<String> = ["-n", "s/a/b/p", "f.txt"].iter().map(|s| s.to_string()).collect();
    assert_eq!(crate::detect_write_targets("sed", &read), crate::WriteTargets::None);
}

#[test]
fn the_loop_guard_counts_only_when_tracewise_started_the_process() {
    let here = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let make = |dir: &std::path::Path, name: &str| {
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        p
    };
    let me = make(here.path(), "tracewise-shim");
    let sibling = make(here.path(), "tracewise");
    let copy = make(elsewhere.path(), "tracewise");
    let shell = make(here.path(), "zsh");
    let applies = |parent: Option<&std::path::Path>| crate::shim_guard_applies(true, parent.and_then(|p| p.to_str()), Some(&me));
    assert!(applies(Some(&sibling)), "a Tracewise binary next to this one");
    assert!(!applies(Some(&copy)), "a copy named tracewise in another folder does not count");
    assert!(!applies(Some(&shell)), "set by a shell or an agent: ignored");
    assert!(!applies(None));
    assert!(!crate::shim_guard_applies(false, sibling.to_str(), Some(&me)));
    assert!(!crate::shim_guard_applies(true, sibling.to_str(), None), "own folder unknown: ignored");
}
