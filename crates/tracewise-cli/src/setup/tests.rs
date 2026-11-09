//! Setup and doctor unit tests.
use super::*;
use std::fs;
use std::path::Path;

fn fake_env() -> (tempfile::TempDir, Env) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let src = tmp.path().join("src");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&src).unwrap();
    for n in PROGRAMS {
        fs::write(src.join(n), format!("program {n}")).unwrap();
    }
    let env = Env { root: home.join(".tracewise"), home, src_dir: src };
    (tmp, env)
}

fn opts(login: Login) -> SetupOptions {
    SetupOptions { login, start: false, shell_rc: true, live: false, profile: "core".to_string() }
}

const OLD_RC: &str = "export FOO=1\n\n# Tracewise git wrapper and shell integration\nexport PATH=\"$HOME/.tracewise/bin:$PATH\"\neval \"$(tracewise shell init zsh)\"\nexport PATH=\"$HOME/.tracewise/bin:$HOME/.tracewise/shims:$PATH\"\nalias ll='ls -l'\n";

#[test]
fn old_lines_become_one_block_and_nothing_else_changes() {
    let rc = rewrite_zshrc(OLD_RC, "# >>> tracewise >>>\nB\n# <<< tracewise <<<\n");
    assert_eq!(rc.matches(".tracewise").count(), 0);
    assert!(rc.starts_with("export FOO=1\n"));
    assert!(rc.contains("alias ll='ls -l'\n"));
    assert_eq!(rc.matches(BLOCK_START).count(), 1);
    assert_eq!(rewrite_zshrc(&rc, "# >>> tracewise >>>\nB\n# <<< tracewise <<<\n"), rc, "twice is the same");
    assert_eq!(strip_tracewise(&rc), "export FOO=1\n\nalias ll='ls -l'\n");
}

#[test]
fn setup_installs_everything_and_uninstall_removes_exactly_that() {
    let (_tmp, env) = fake_env();
    fs::write(env.zshrc(), OLD_RC).unwrap();
    fs::create_dir_all(env.home.join(".cargo/bin")).unwrap();
    fs::write(env.home.join(".cargo/bin/tracewise"), "old copy").unwrap();
    let m = setup_in(&env, &opts(Login::Yes)).unwrap();
    assert!(env.home.join(".cargo/bin/tracewise").is_symlink(), "the old copy now links to the installed one");
    assert_eq!(m.programs.len(), PROGRAMS.len());
    for n in PROGRAMS {
        assert!(env.bin().join(n).is_file(), "{n}");
    }
    assert!(env.bin().join("git").is_symlink());
    assert_eq!(fs::read_dir(env.shims()).unwrap().count(), crate::cmd_shims::SHIM_TOOLS.len());
    let rc = fs::read_to_string(env.zshrc()).unwrap();
    assert_eq!(rc.matches(BLOCK_START).count(), 1);
    assert!(m.zshrc_backup.is_some() && Path::new(m.zshrc_backup.as_ref().unwrap()).is_file());
    let zshenv = fs::read_to_string(env.zshenv()).unwrap();
    assert!(zshenv.contains(".tracewise/bin") && !zshenv.contains("shell init"), "PATH is in .zshenv: {zshenv}");
    assert!(rc.contains("shell init zsh") && !rc.contains("export PATH"), "only the hook is in .zshrc: {rc}");
    assert!(env.agent_plist().is_file());
    assert!(env.manifest().is_file());

    // again: nothing changes, no second backup
    let again = setup_in(&env, &opts(Login::Yes)).unwrap();
    assert_eq!(again.zshrc_backup, m.zshrc_backup);
    assert_eq!(fs::read_to_string(env.zshrc()).unwrap(), rc);
    assert_eq!(fs::read_to_string(env.zshenv()).unwrap(), zshenv);

    uninstall_in(&env, false, false);
    assert!(!env.home.join(".cargo/bin/tracewise").exists());
    assert!(!env.bin().exists() && !env.shims().exists() && !env.manifest().exists());
    assert!(!env.agent_plist().exists());
    assert_eq!(fs::read_to_string(env.zshrc()).unwrap(), "export FOO=1\n\nalias ll='ls -l'\n");
}

#[test]
fn login_no_leaves_no_login_item_and_a_reinstall_can_remove_it() {
    let (_tmp, env) = fake_env();
    setup_in(&env, &opts(Login::Yes)).unwrap();
    assert!(env.agent_plist().is_file());
    let m = setup_in(&env, &opts(Login::No)).unwrap();
    assert!(m.launch_agent.is_none() && !env.agent_plist().exists());
}

#[test]
fn doctor_names_what_is_missing_and_stray_lines() {
    let (_tmp, env) = fake_env();
    fs::write(env.zshrc(), OLD_RC).unwrap();
    let before = checks(&env, "/usr/bin");
    assert!(before.iter().any(|c| !c.ok && c.what.contains("install.json")));
    assert!(before.iter().any(|c| !c.ok && c.what.contains("old Tracewise line")));
    setup_in(&env, &opts(Login::No)).unwrap();
    let path = format!("{}:{}:/usr/bin", env.bin().display(), env.shims().display());
    let after = checks(&env, &path);
    let failing: Vec<_> = after.iter().filter(|c| !c.ok && c.what != "daemon running").map(|c| c.what.clone()).collect();
    assert!(failing.is_empty(), "{failing:?}");
}

#[test]
fn a_missing_tracewise_program_is_an_error() {
    let (_tmp, env) = fake_env();
    fs::remove_file(env.src_dir.join("tracewise")).unwrap();
    assert!(setup_in(&env, &opts(Login::No)).is_err());
}

#[test]
fn setup_profile_writes_config_idempotently() {
    let (_tmp, env) = fake_env();
    let m1 = setup_in(&env, &opts(Login::No)).unwrap();
    assert_eq!(m1.profile, "core");
    let cfg = fs::read_to_string(env.root.join("config.toml")).unwrap();
    assert_eq!(cfg, "[profile]\nname = \"core\"\n");
    let m2 = setup_in(&env, &opts(Login::No)).unwrap();
    assert_eq!(m2.profile, "core");
    assert_eq!(fs::read_to_string(env.root.join("config.toml")).unwrap(), cfg);
}

#[test]
fn doctor_compares_programs_with_the_candidate_checksums() {
    let (_tmp, env) = fake_env();
    let names: Vec<&str> = PROGRAMS.to_vec();
    let sums = std::process::Command::new("shasum").args(["-a", "256"]).args(&names).current_dir(&env.src_dir).output().unwrap();
    fs::write(env.src_dir.join(CHECKSUM_FILE), &sums.stdout).unwrap();
    setup_in(&env, &opts(Login::Yes)).unwrap();
    let find = |env: &Env| doctor::checks(env, "").into_iter().find(|c| c.what.contains("checksums"));
    assert!(find(&env).is_some_and(|c| c.ok), "matching programs pass");
    fs::write(env.bin().join("tracewise-shim"), "tampered").unwrap();
    let c = find(&env).unwrap();
    assert!(!c.ok && c.what.contains("tracewise-shim"), "{}", c.what);
}

#[test]
fn fish_gets_one_file_that_setup_writes_and_uninstall_removes_and_a_foreign_file_is_left_alone() {
    let (_tmp, env) = fake_env();
    fs::create_dir_all(env.home.join(".config/fish")).unwrap(); // fish is installed
    setup_in(&env, &opts(Login::No)).unwrap();
    let file = install::fish_file(&env);
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains(BLOCK_START) && text.contains("tracewise shell init fish | source"), "{text}");
    assert!(text.contains("$HOME/.tracewise/shims") && text.contains("$HOME/.tracewise/bin"), "{text}");
    setup_in(&env, &opts(Login::No)).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), text, "twice is the same");
    uninstall_in(&env, false, false);
    assert!(!file.exists(), "the file was only Tracewise's");

    // a tracewise.fish the person wrote is never overwritten or removed
    fs::write(&file, "# mine\nset -gx FOO 1\n").unwrap();
    setup_in(&env, &opts(Login::No)).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "# mine\nset -gx FOO 1\n");
    uninstall_in(&env, false, false);
    assert!(file.exists());
}

#[test]
fn without_fish_no_fish_file_is_written() {
    let (_tmp, env) = fake_env();
    if install::user_shell() == "fish" {
        return; // the machine running the tests uses fish: the file is expected there
    }
    setup_in(&env, &opts(Login::No)).unwrap();
    assert!(!install::fish_file(&env).exists());
}
