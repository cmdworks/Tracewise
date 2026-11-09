use super::*;

#[test]
fn only_reads_knows_readers() {
    for c in ["ls", "sleep 40", "git status && git log -3", "cat a.txt | wc -l", "trail log --limit 3", "cd x && pwd"] {
        assert!(only_reads(c), "{c}");
    }
    for c in ["echo hi > f", "git checkout x", "npm install", "sed -i s/a/b/ f", "python3 gen.py", "ls; rm x", ""] {
        assert!(!only_reads(c), "{c}");
    }
}

#[test]
fn test_predict_redirection() {
    let files = predict_written_files("printf 'generated\\n' > out.txt");
    assert_eq!(files, vec![PathBuf::from("out.txt")]);

    let files2 = predict_written_files("echo foo >> append.log");
    assert_eq!(files2, vec![PathBuf::from("append.log")]);
}

#[test]
fn test_predict_pipe_and_tee() {
    let files = predict_written_files("cat file.txt | tee output.txt");
    assert_eq!(files, vec![PathBuf::from("output.txt")]);
}

#[test]
fn test_predict_touch_and_cp() {
    let files = predict_written_files("touch a.txt b.txt");
    assert_eq!(files, vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]);

    let files2 = predict_written_files("cp src.txt dst.txt");
    assert_eq!(files2, vec![PathBuf::from("dst.txt")]);
}

#[test]
fn a_greater_than_inside_quotes_is_not_a_redirect() {
    let files = predict_written_files("sed -i '' \"s/=> '$' + n/=> 'USD ' + n/\" a.js");
    assert_eq!(files, vec![PathBuf::from("a.js")]);
    assert!(predict_written_files("grep 'a>b' f.txt 2>&1").is_empty());
}

#[test]
fn flag_and_attribute_commands_name_their_files() {
    let one = |c: &str| predict_written_files(c);
    assert_eq!(one("chflags nouchg f.txt"), vec![PathBuf::from("f.txt")]);
    assert_eq!(one("/usr/bin/chflags -R nouchg a b"), vec![PathBuf::from("a"), PathBuf::from("b")]);
    assert_eq!(one("sudo chflags noschg f.txt"), vec![PathBuf::from("f.txt")]);
    assert_eq!(one("chmod 666 f.txt"), vec![PathBuf::from("f.txt")]);
    assert_eq!(one("xattr -c f.txt"), vec![PathBuf::from("f.txt")]);
    assert_eq!(one("xattr -d com.apple.quarantine f.txt"), vec![PathBuf::from("f.txt")]);
    assert!(one("xattr -l f.txt").is_empty());
    assert!(one("xattr f.txt").is_empty());
    assert_eq!(one("env X=1 command sed -i '' s/a/b/ f.txt"), vec![PathBuf::from("f.txt")]);
}

#[test]
fn background_starters_are_found() {
    let lctl_load = format!("{} load /tmp/x.plist", tracewise_platform::service::LAUNCHCTL);
    let lctl_list = format!("{} list", tracewise_platform::service::LAUNCHCTL);
    for c in ["nohup sh -c 'sleep 1' &", "sleep 5 &", &lctl_load, "crontab -", "echo x | at now", "sh -c \"nohup x &\"", "setsid foo"] {
        assert!(starts_background(c), "{c}");
    }
    for c in ["python3 -c 'x'", "ls && echo a", &lctl_list, "crontab -l", "sh -c 'ls'"] {
        assert!(!starts_background(c), "{c}");
    }
}

#[test]
fn nested_shells_are_classified_by_their_script() {
    assert_eq!(predict_written_files("sh -c \"sed -i '' s/a/b/ g.txt\""), vec![PathBuf::from("g.txt")]);
    assert_eq!(predict_written_files("bash -lc 'rm a.txt'"), vec![PathBuf::from("a.txt")]);
}

#[test]
fn commands_with_unknown_targets_are_found() {
    let lctl_load = format!("{} load /tmp/x.plist", tracewise_platform::service::LAUNCHCTL);
    let lctl_list = format!("{} list", tracewise_platform::service::LAUNCHCTL);
    for c in [
        "python3 -c \"open('f','w')\"",
        "node -e \"1\"",
        "perl -e 'print 1'",
        "ruby -e 'puts 1'",
        "echo g | xargs sed -i '' s/a/b/",
        "nohup sh -c 'sleep 1' &",
        &lctl_load,
        "crontab -",
        "echo x | at now",
        "sh -c \"python3 -c 'x'\"",
        "sleep 5 &",
    ] {
        assert!(may_write_anywhere(c), "{c}");
    }
    for c in ["python3 script.py", "grep a f && ls", &lctl_list, "crontab -l", "echo a && echo b", "xargs grep foo"] {
        assert!(!may_write_anywhere(c), "{c}");
    }
}

#[test]
fn test_predict_sed() {
    let files = predict_written_files("sed -i 's/foo/bar/g' target.txt");
    assert_eq!(files, vec![PathBuf::from("target.txt")]);
}

#[test]
fn test_predict_git() {
    let files = predict_written_files("git checkout -- foo.rs bar.rs");
    assert_eq!(files, vec![PathBuf::from("foo.rs"), PathBuf::from("bar.rs")]);
}
