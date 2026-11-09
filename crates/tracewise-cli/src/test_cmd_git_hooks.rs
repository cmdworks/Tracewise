use super::*;
use tempfile::tempdir;

fn repo(tmp: &Path) -> RepoPaths {
    let git = std::process::Command::new("git").arg("init").arg("-q").arg(tmp).env("TRAIL_GIT_DEPTH", "1").status().unwrap();
    assert!(git.success());
    let g = tmp.join(".git");
    RepoPaths { work_tree: tmp.to_path_buf(), git_dir: g.clone(), git_common_dir: g }
}

#[test]
fn the_script_skips_when_the_wrapper_started_git_and_only_the_commit_gate_can_stop_git() {
    let pre = script("pre-commit", "/x/trail");
    assert!(pre.contains("[ -z \"$TRAIL_GIT_DEPTH\" ]") && pre.contains("exit 1") && pre.contains("pre-commit.pre-trail"));
    let post = script("post-checkout", "/x/trail");
    assert!(!post.contains("exit 1"), "a post hook never stops git");
    assert!(script("post-merge", "/it's/trail").contains(r"'/it'\''s/trail'"), "a quote in the path is escaped");
}

#[test]
fn install_chains_an_existing_hook_is_idempotent_and_uninstall_puts_it_back() {
    let tmp = tempdir().unwrap();
    let r = repo(tmp.path());
    let hooks = r.git_common_dir.join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(hooks.join("pre-commit"), "#!/bin/sh\necho mine\n").unwrap();
    tracewise_platform::private::set_mode(hooks.join("pre-commit"), 0o755).unwrap();

    install(&r).unwrap();
    install(&r).unwrap(); // twice: the person's hook is not moved over by Trail's own
    let st: std::collections::HashMap<_, _> = status(&r).into_iter().collect();
    assert_eq!(st["pre-commit"], "trail+previous");
    assert_eq!(st["post-commit"], "trail");
    assert_eq!(fs::read_to_string(hooks.join("pre-commit.pre-trail")).unwrap(), "#!/bin/sh\necho mine\n");
    for h in HOOKS {
        assert!(fs::read_to_string(hooks.join(h)).unwrap().contains(MARK));
    }

    uninstall(&r);
    assert_eq!(fs::read_to_string(hooks.join("pre-commit")).unwrap(), "#!/bin/sh\necho mine\n");
    assert!(!hooks.join("pre-commit.pre-trail").exists());
    assert!(!hooks.join("post-commit").exists());
    assert!(status(&r).iter().all(|(n, s)| if *n == "pre-commit" { *s == "other" } else { *s == "none" }));
}

#[test]
fn a_custom_hooks_path_is_left_alone() {
    let tmp = tempdir().unwrap();
    let r = repo(tmp.path());
    let ok = std::process::Command::new("git").arg("-C").arg(tmp.path()).args(["config", "core.hooksPath", ".husky"]).status().unwrap();
    assert!(ok.success());
    let err = install(&r).unwrap_err();
    assert!(err.contains("core.hooksPath"), "{err}");
    assert!(!r.git_common_dir.join("hooks/pre-commit").exists());
}
