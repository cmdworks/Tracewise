//! Shared helper for the integration tests: a throw-away git repository with Tracewise initialised, Tracewise's own home folder outside
//! the repository (so it is not recorded as changes), and an environment that does not depend on who runs the tests.
//!
//! Every call runs the real `trail` binary. The environment is cleaned of anything that would change who a change is attributed
//! to (agent markers, `TRAIL_ACTOR`, `TRAIL_WHY`, ...), so the tests give the same answer inside an agent session and in CI.
#![allow(dead_code)]
use std::path::Path;
use std::process::{Command, Stdio};

/// Variables that change attribution, reasons or hook behaviour; removed from every call.
const AMBIENT: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_SESSION_ID",
    "ANTIGRAVITY_AGENT",
    "ANTIGRAVITY_CONVERSATION_ID",
    "TRAIL_ACTOR",
    "TRAIL_WHY",
    "TRAIL_NOW", // an agent cannot freeze the clock, so the tests use the real one
    "TRAIL_WARN",
    "TRAIL_PROMPT_LINE",
    "TRAIL_SHELL_ACTIVE",
    "TRAIL_SHIM_DEPTH",
];

pub struct Run {
    pub out: String,
    pub err: String,
    pub code: i32,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.out).unwrap_or_else(|e| panic!("not JSON ({e}): {}\nstderr: {}", self.out, self.err))
    }
}

pub struct Repo {
    tmp: tempfile::TempDir,
    home: tempfile::TempDir,
}

impl Repo {
    /// A git repository with Tracewise initialised. `config` is the content of `.trailconfig` (empty for none).
    pub fn new(config: &str) -> Self {
        let repo = Self::uninitialised(config);
        assert!(repo.me(&["init"]).ok(), "trail init failed");
        repo
    }

    /// Like `new` but before `trail init`, so a test can create files that become the baseline.
    pub fn uninitialised(config: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let ok = Command::new("git").args(["init", "-q"]).current_dir(tmp.path()).status().unwrap().success();
        assert!(ok, "git init failed");
        if !config.is_empty() {
            std::fs::write(tmp.path().join(".trailconfig"), config).unwrap();
        }
        Self { tmp, home: tempfile::tempdir().unwrap() }
    }

    pub fn dir(&self) -> &Path {
        self.tmp.path()
    }

    /// Tracewise's home folder for this repository (outside the repository).
    pub fn home(&self) -> &Path {
        self.home.path()
    }

    pub fn write(&self, file: &str, content: &str) {
        let path = self.dir().join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    pub fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.dir().join(file)).unwrap()
    }

    /// Waits until the clock is at least a millisecond past the newest record, so whatever happens next is strictly newer.
    /// (Records carry millisecond times; two steps can otherwise share one.)
    pub fn later(&self) {
        let newest = self.rows(&[]).iter().filter_map(|c| c["time"]["start"].as_str().map(String::from)).max().unwrap_or_default();
        let newest = chrono::DateTime::parse_from_rfc3339(&newest).map(|t| t.timestamp_millis()).unwrap_or(0);
        while chrono::Utc::now().timestamp_millis() <= newest + 1 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Runs `trail` as `actor` (`human:me`, `agent:codex:s1`), with optional stdin and extra environment.
    pub fn with(&self, actor: &str, args: &[&str], stdin: Option<&str>, env: &[(&str, &str)]) -> Run {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_trail"));
        cmd.args(args).current_dir(self.dir()).env("TRAIL_HOME", self.home.path()).env("TRAIL_ACTOR", actor);
        for name in AMBIENT.iter().filter(|n| **n != "TRAIL_ACTOR") {
            cmd.env_remove(name);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        std::io::Write::write_all(&mut child.stdin.take().unwrap(), stdin.unwrap_or("").as_bytes()).ok();
        let out = child.wait_with_output().unwrap();
        Run {
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code().unwrap_or(-1),
        }
    }

    /// Starts `trail` under another program name (`sed`, `rm`): how a command shim calls it. `env` is added to the environment.
    pub fn as_program(&self, program: &str, args: &[&str], env: &[(&str, &str)]) -> Run {
        use std::os::unix::process::CommandExt;
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_trail"));
        cmd.arg0(program).args(args).current_dir(self.dir()).env("TRAIL_HOME", self.home.path());
        for name in AMBIENT {
            cmd.env_remove(name);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = cmd.output().unwrap();
        Run {
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code().unwrap_or(-1),
        }
    }

    pub fn run(&self, actor: &str, args: &[&str]) -> Run {
        self.with(actor, args, None, &[])
    }

    /// Runs as a person.
    pub fn me(&self, args: &[&str]) -> Run {
        self.run("human:me", args)
    }

    /// The rows of `trail log --json` with extra arguments.
    pub fn rows(&self, extra: &[&str]) -> Vec<serde_json::Value> {
        let mut args = vec!["log", "--json"];
        args.extend_from_slice(extra);
        let run = self.me(&args);
        assert!(run.ok(), "log failed: {}", run.err);
        run.json().as_array().unwrap().clone()
    }

    /// Strictly later than everything so far: writes `content`, scans as `actor`, returns the id of the newest change of `file`.
    pub fn edit_as(&self, actor: &str, file: &str, content: &str) -> String {
        self.later();
        self.write(file, content);
        let scan = self.run(actor, &["scan"]);
        assert!(scan.ok(), "scan failed: {}", scan.err);
        self.rows(&[]).iter().find(|c| c["path"] == file).unwrap_or_else(|| panic!("no change for {file}"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    }
}
