//! Health checks and diagnostics for tracewise setup.
use super::*;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Check {
    pub ok: bool,
    pub warn: bool,
    pub what: String,
    pub fix: String,
}

pub(crate) fn check(ok: bool, what: impl Into<String>, fix: impl Into<String>) -> Check {
    Check { ok, warn: false, what: what.into(), fix: fix.into() }
}

pub(crate) fn warn(what: impl Into<String>, fix: impl Into<String>) -> Check {
    Check { ok: true, warn: true, what: what.into(), fix: fix.into() }
}

pub(crate) fn first_on_path(path_var: &str, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path_var).map(|d| d.join(name)).find(|p| p.exists())
}

pub fn checks(env: &Env, path_var: &str) -> Vec<Check> {
    let mut out = Vec::new();
    let manifest = read_manifest(env);
    out.push(check(manifest.is_some(), "install.json: set up by `tracewise setup`", "run `tracewise setup`"));

    check_programs_and_git(env, &mut out);
    check_shims_and_blocks(env, &mut out);
    check_fish(env, &mut out);
    check_path_and_daemon(env, path_var, &mut out);
    check_platform_features(&mut out);

    check_database_size(&mut out);
    check_adapters_and_config(&mut out);

    let profile = tracewise_core::profile::ProfileConfig::from_cwd();
    out.push(Check { ok: true, warn: false, what: format!("profile: {}", profile.description()), fix: String::new() });

    out
}

/// Warns about a broken adapter file or a broken global `config.toml`; both fail open, so a warning is all.
fn check_adapters_and_config(out: &mut Vec<Check>) {
    let work_tree = std::env::current_dir().ok().and_then(|d| tracewise_git::RepoPaths::discover(&d)).map(|r| r.work_tree);
    for (name, _, err) in crate::hook::adapter::list_files(work_tree.as_deref()) {
        if let Some(e) = err {
            out.push(warn(format!("adapter {name} is broken and ignored: {e}"), "fix or delete the file (see `tracewise agent check`)"));
        }
    }
    let config = tracewise_core::get_tracewise_home().join("config.toml");
    if let Ok(body) = fs::read_to_string(&config) {
        if let Err(e) = toml::from_str::<toml::Table>(&body) {
            out.push(warn(
                format!("{} is not valid TOML and ignored: {}", config.display(), e.to_string().replace('\n', " ")),
                "fix the file; the defaults apply meanwhile",
            ));
        }
    }
}

/// Warns when the history database of the current repository is over 50 MB (the plan's limit).
fn check_database_size(out: &mut Vec<Check>) {
    let Some(repo) = std::env::current_dir().ok().and_then(|d| tracewise_git::RepoPaths::discover(&d)) else { return };
    let bytes: u64 = std::fs::read_dir(repo.data_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("tracewise.db"))
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    if bytes > 50 << 20 {
        out.push(warn(
            format!("database is {} MB (over 50)", bytes >> 20),
            "run `tracewise gc`; if it grows again, check `tracewise commands` for a noisy tool",
        ));
    }
}

fn check_platform_features(out: &mut Vec<Check>) {
    if let Err(e) = tracewise_platform::sensor::check_sensor_supported() {
        out.push(Check { ok: true, warn: true, what: format!("strict sensor: {e}"), fix: String::new() });
    }

    if let Err(e) = tracewise_platform::service::check_service_management_supported() {
        out.push(Check { ok: true, warn: true, what: format!("service management: {e}"), fix: String::new() });
    }
}

fn check_programs_and_git(env: &Env, out: &mut Vec<Check>) {
    for name in PROGRAMS.iter().filter(|n| **n != "tracewise-sensor") {
        let p = env.bin().join(name);
        out.push(check(p.is_file(), format!("program {}", p.display()), "run `tracewise setup`"));
    }
    let sensor = env.bin().join("tracewise-sensor");
    if !sensor.is_file() {
        out.push(warn("program tracewise-sensor is not installed (only the strict mode needs it)", "build it, then run `tracewise setup`"));
    }
    check_binary_hashes(&env.bin(), out);
    let git = env.bin().join("git");
    out.push(check(git.exists(), "git wrapper ~/.tracewise/bin/git", "run `tracewise setup`"));
}

/// With a `SHA256SUMS` next to the programs (a release candidate), every listed program must still match it.
/// Uses the system `shasum`; a machine without it gets a warning, not a failure.
fn check_binary_hashes(bin: &Path, out: &mut Vec<Check>) {
    if !bin.join(CHECKSUM_FILE).is_file() {
        return;
    }
    let run = std::process::Command::new("shasum").args(["-a", "256", "-c", CHECKSUM_FILE]).current_dir(bin).output();
    match run {
        Ok(o) if o.status.success() => out.push(check(true, "program checksums match SHA256SUMS", "")),
        Ok(o) => {
            let bad: Vec<String> =
                String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.ends_with(": OK")).map(str::to_string).collect();
            out.push(check(false, format!("program checksums differ from SHA256SUMS: {}", bad.join("; ")), "reinstall from the candidate"));
        }
        Err(_) => out.push(warn("cannot run shasum, so the program checksums were not compared", "install shasum")),
    }
}

fn check_shims_and_blocks(env: &Env, out: &mut Vec<Check>) {
    let shims = crate::cmd_shims::SHIM_TOOLS.iter().filter(|t| env.shims().join(t).exists()).count();
    out.push(check(
        shims >= crate::cmd_shims::SHIM_TOOLS.len(),
        format!("shims: {shims} of {} work", crate::cmd_shims::SHIM_TOOLS.len()),
        "run `tracewise setup`",
    ));

    for (path, label) in [(env.zshenv(), "~/.zshenv (PATH for every shell)"), (env.zshrc(), "~/.zshrc (terminal hook)")] {
        let text = fs::read_to_string(&path).unwrap_or_default();
        out.push(check(text.lines().any(|l| l.trim() == BLOCK_START), format!("{label} has the Tracewise block"), "run `tracewise setup`"));
        let legacy = text
            .lines()
            .scan(false, |inside, l| {
                if l.trim() == BLOCK_START {
                    *inside = true;
                }
                let stray = !*inside && is_legacy_line(l);
                if l.trim() == BLOCK_END {
                    *inside = false;
                }
                Some(stray)
            })
            .filter(|s| *s)
            .count();
        if legacy > 0 {
            out.push(check(
                false,
                format!("{} has {legacy} old Tracewise line(s) outside the block", path.display()),
                "run `tracewise setup` (it folds them into the block)",
            ));
        }
    }
}

/// fish has no block in an rc file: Tracewise owns `conf.d/tracewise.fish`.
fn check_fish(env: &Env, out: &mut Vec<Check>) {
    use super::install::{fish_file, fish_installed};
    if !fish_installed(env) {
        return;
    }
    let text = fs::read_to_string(fish_file(env)).unwrap_or_default();
    out.push(check(text.contains(BLOCK_START), "~/.config/fish/conf.d/tracewise.fish has the Tracewise block", "run `tracewise setup`"));
}

fn check_path_and_daemon(env: &Env, path_var: &str, out: &mut Vec<Check>) {
    let bin = env.bin();
    out.push(check(
        first_on_path(path_var, "git").is_some_and(|p| p.starts_with(&bin)),
        "this shell finds the Tracewise git wrapper first",
        "open a new terminal (or `source ~/.zshrc`) after `tracewise setup`",
    ));
    if let Some(t) = first_on_path(path_var, "tracewise") {
        if !t.starts_with(&bin) {
            out.push(warn(
                format!("`tracewise` on PATH is {}, not the installed copy", t.display()),
                "remove that copy or put ~/.tracewise/bin first",
            ));
        }
    }
    out.push(check(crate::util::is_daemon_running(), "daemon running", "run `tracewise daemon start`"));
    let repos = tracewise_core::repos::list_in(&env.root);
    if repos.is_empty() {
        out.push(warn(
            "no repository is registered with the daemon",
            "run `tracewise init` (or `tracewise daemon start`) inside a repository",
        ));
    } else {
        let names: Vec<_> = repos.iter().map(|r| r.work_tree.display().to_string()).collect();
        out.push(Check {
            ok: true,
            warn: false,
            what: format!("{} repositor{} watched: {}", repos.len(), if repos.len() == 1 { "y" } else { "ies" }, names.join(", ")),
            fix: String::new(),
        });
    }
    let login = env.agent_plist().is_file();
    out.push(Check {
        ok: true,
        warn: false,
        what: format!(
            "daemon starts at login: {}",
            if login { "yes" } else { "no (your choice; `tracewise setup --login yes` changes it)" }
        ),
        fix: String::new(),
    });
}
