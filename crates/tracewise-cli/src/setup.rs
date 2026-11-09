//! `trail setup`, `trail uninstall`, `trail doctor --all`: one root (`TRAIL_HOME`), one manifest (`install.json`),
//! one marked block in `~/.zshrc`. Nothing here needs a repository, and nothing needs sudo (the strict sensor has
//! its own `trail strict install`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// Written next to the programs by `scripts/package.sh`; absent for a build from source.
pub(crate) const CHECKSUM_FILE: &str = "SHA256SUMS";
pub(crate) const PROGRAMS: &[&str] = &["trail", "trail-git", "trail-shim", "trail-watchd", "trail-sensor"];
pub(crate) const BLOCK_START: &str = "# >>> trail >>>";
pub(crate) const BLOCK_END: &str = "# <<< trail <<<";

pub(crate) mod doctor;
pub(crate) mod install;
pub(crate) mod login;
#[cfg(test)]
mod tests;

pub use doctor::*;
pub use install::*;
pub(crate) use login::*;

/// Where things are. Tests point all three at temporary folders.
pub struct Env {
    pub home: PathBuf,
    pub root: PathBuf,
    /// The folder holding the programs to install (next to the running `trail`).
    pub src_dir: PathBuf,
}

impl Env {
    pub fn current() -> Env {
        let exe = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok()).unwrap_or_default();
        Env {
            home: std::env::var("HOME").map(PathBuf::from).unwrap_or_default(),
            root: tracewise_core::get_trail_home(),
            src_dir: exe.parent().map(Path::to_path_buf).unwrap_or_default(),
        }
    }
    pub(crate) fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }
    pub(crate) fn shims(&self) -> PathBuf {
        self.root.join("shims")
    }
    pub(crate) fn manifest(&self) -> PathBuf {
        self.root.join("install.json")
    }
    pub(crate) fn zshrc(&self) -> PathBuf {
        self.home.join(".zshrc")
    }
    /// Read by every zsh, including the shells an agent starts: the PATH lives here so hooks find `trail`.
    pub(crate) fn zshenv(&self) -> PathBuf {
        self.home.join(".zshenv")
    }
    pub(crate) fn agent_plist(&self) -> PathBuf {
        self.home.join("Library/LaunchAgents").join(format!("{AGENT_LABEL}.plist"))
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Manifest {
    pub version: String,
    pub installed_at: String,
    #[serde(default)]
    pub profile: String,
    pub programs: Vec<String>,
    pub shims: usize,
    pub zshrc_block: bool,
    pub zshenv_block: bool,
    pub zshrc_backup: Option<String>,
    pub launch_agent: Option<String>,
    /// Old copies in ~/.cargo/bin that now link to the installed programs (so they never go stale).
    pub cargo_links: Vec<String>,
}

pub(crate) fn read_manifest(env: &Env) -> Option<Manifest> {
    serde_json::from_str(&fs::read_to_string(env.manifest()).ok()?).ok()
}

pub(crate) fn write_manifest(env: &Env, m: &Manifest) -> Result<(), String> {
    let text = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    tracewise_platform::private::write(env.manifest(), text + "\n").map_err(|e| format!("cannot write install.json: {e}"))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Login {
    Ask,
    Yes,
    No,
}

pub(crate) fn ask_yes_no(prompt: &str) -> bool {
    use std::io::Write;
    print!("{prompt} [Y/n] ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return true;
    }
    let t = line.trim().to_lowercase();
    t.is_empty() || t == "y" || t == "yes"
}

pub struct SetupOptions {
    pub login: Login,
    pub start: bool,
    pub shell_rc: bool,
    /// Load the login item and start the daemon (off in tests).
    pub live: bool,
    pub profile: String,
}

/// Tests set `TRAIL_SETUP_NO_LAUNCHCTL=1`: they must not load, unload or kill anything on the machine they run on.
pub(crate) fn live() -> bool {
    std::env::var("TRAIL_SETUP_NO_LAUNCHCTL").as_deref() != Ok("1")
}

pub fn run_setup(json: bool, login: Login, no_start: bool, no_shell_rc: bool, strict: bool, events: &str, profile: &str) {
    let env = Env::current();
    let opts = SetupOptions { login, start: !no_start, shell_rc: !no_shell_rc, live: live(), profile: profile.to_string() };
    match setup_in(&env, &opts) {
        Ok(m) => {
            if json {
                println!("{}", serde_json::to_string(&m).unwrap_or_default());
            } else {
                println!("Trail {} is set up in {}", m.version, env.root.display());
                println!("  programs: {}", m.programs.join(", "));
                println!("  shims:    {} in {}", m.shims, env.shims().display());
                if m.zshrc_block {
                    println!("  shell:    one Trail block in ~/.zshenv (PATH) and one in ~/.zshrc (terminal hook)");
                }
                if m.zshrc_block && install::fish_installed(&env) && install::fish_file(&env).is_file() {
                    println!("  shell:    fish: one Trail file, {}", install::fish_file(&env).display());
                }
                if let Some(shell) = install::unsupported_shell().filter(|_| m.zshrc_block) {
                    println!(
                        "  shell:    {shell} is not supported yet: add {}/bin and {}/shims to PATH by hand",
                        env.root.display(),
                        env.root.display()
                    );
                }
                if let Some(b) = &m.zshrc_backup {
                    println!("  backup:   {b}");
                }
                println!(
                    "  login:    {}",
                    if m.launch_agent.is_some() { "daemon starts at login" } else { "daemon is started by hand (`trail daemon start`)" }
                );
                println!("Open a new terminal, then run `trail doctor --all`.");
            }
        }
        Err(e) => {
            eprintln!("trail: setup failed: {e}");
            process::exit(1);
        }
    }
    if strict {
        let status = Command::new(env.bin().join("trail")).args(["strict", "install", "--events", events]).status();
        if !status.map(|s| s.success()).unwrap_or(false) {
            process::exit(1);
        }
    }
}

pub fn run_uninstall(json: bool, purge: bool) {
    let env = Env::current();
    let removed = uninstall_in(&env, purge, live());
    if json {
        println!("{}", serde_json::json!({ "removed": removed }));
    } else {
        if removed.is_empty() {
            println!("nothing to remove: Trail is not set up here");
        }
        for r in &removed {
            println!("removed {r}");
        }
        println!(
            "Repository histories (.git/trail) are untouched. The strict sensor, if installed, is removed with `trail strict uninstall`."
        );
    }
}

pub fn run_doctor_all(json: bool) {
    let env = Env::current();
    let path_var = std::env::var("PATH").unwrap_or_default();
    let list = checks(&env, &path_var);
    #[cfg(feature = "platform-stub")]
    let bad = 0;
    #[cfg(not(feature = "platform-stub"))]
    let bad = list.iter().filter(|c| !c.ok).count();
    if json {
        let items: Vec<_> = list.iter().map(|c| serde_json::json!({"ok": c.ok, "warn": c.warn, "what": c.what, "fix": c.fix})).collect();
        println!("{}", serde_json::json!({ "ok": bad == 0, "checks": items }));
    } else {
        for c in &list {
            let mark = if !c.ok {
                "FAIL"
            } else if c.warn {
                "warn"
            } else {
                "ok  "
            };
            if c.fix.is_empty() || (c.ok && !c.warn) {
                println!("{mark}  {}", c.what);
            } else {
                println!("{mark}  {}  ->  {}", c.what, c.fix);
            }
        }
        println!("{}", if bad == 0 { "all good" } else { "problems found" });
    }
    if bad > 0 {
        process::exit(1);
    }
}
