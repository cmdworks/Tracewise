use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "trail", version, about = "Tracewise: audit trail and coordination for developer agents and humans")]
pub struct Cli {
    #[arg(short = 'C', help = "Run as if tracewise was started in <dir> instead of the current working directory")]
    pub directory: Option<PathBuf>,
    #[arg(long, global = true, help = "Output in JSON format")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Initialize trail in the current repository
    Init,
    /// Show trail status
    Status,
    /// Check repository health and report stuck immutable files; `--all` checks the whole installation
    #[command(after_help = "Examples:\n  trail doctor\n  trail doctor --all")]
    Doctor {
        #[arg(long, help = "Check the whole Trail installation, not only this repository")]
        all: bool,
    },
    /// Install Tracewise for this user: programs, shims, git wrapper, one block in ~/.zshrc (no sudo)
    #[command(after_help = "Examples:\n  trail setup\n  trail setup --login yes --profile core")]
    Setup {
        #[arg(long, value_parser = ["yes", "no", "ask"], default_value = "ask", help = "Start the daemon at login")]
        login: String,
        #[arg(long = "no-start", help = "Do not start the daemon now")]
        no_start: bool,
        #[arg(long = "no-shell-rc", help = "Do not touch ~/.zshrc")]
        no_shell_rc: bool,
        #[arg(long, help = "Also install the strict sensor (asks for sudo)")]
        strict: bool,
        #[arg(long, default_value = "light", help = "With --strict: light or full")]
        events: String,
        #[arg(long, default_value = "core", help = "Default profile (core or full)")]
        profile: String,
    },
    /// Remove what `trail setup` installed (repository histories stay)
    Uninstall {
        #[arg(long, help = "Also delete the whole ~/.trail folder")]
        purge: bool,
    },
    /// Show audit log
    #[command(after_help = "Examples:\n  trail log\n  trail log src/main.rs\n  trail log --json")]
    Log {
        #[arg(help = "Filter changes by path")]
        path: Option<String>,
        #[arg(long, help = "Maximum number of records to return")]
        limit: Option<usize>,
        #[arg(long, help = "Filter by actor name")]
        actor: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Filter since duration or time")]
        since: Option<String>,
        #[arg(long, help = "Only changes whose path or reason contains this text (case-insensitive)")]
        grep: Option<String>,
        #[arg(long, help = "Only changes newer than this change id (an app asks for what is new since the last row it has)")]
        after: Option<String>,
        #[arg(long, help = "Cluster the changes by task (intent, else session)")]
        group: bool,
    },
    /// Show details and diff of a change
    Show {
        #[arg(help = "The change ID (ULID)")]
        id: String,
    },
    /// One task as Markdown, for a review or a hand-off
    #[command(after_help = "Examples:\n  trail report --session <session id>\n  trail report --intent <intent id> --json")]
    Report {
        #[arg(long, help = "Changes by this actor")]
        actor: Option<String>,
        #[arg(long, help = "Changes under this intent id")]
        intent: Option<String>,
        #[arg(long, help = "Changes of this session id")]
        session: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Only changes since this duration or time")]
        since: Option<String>,
    },
    /// Who holds a file and who changed it lately
    #[command(after_help = "Examples:\n  trail file src/main.rs\n  trail file src/main.rs --limit 3 --json")]
    File {
        #[arg(help = "Path to the file, relative to the repository")]
        path: String,
        #[arg(short, long, default_value_t = 10, help = "How many changes to show")]
        limit: usize,
    },
    /// Scan repository for changes
    Scan,
    /// Show line history blame for a file
    #[command(after_help = "Examples:\n  trail blame src/main.rs")]
    Blame {
        #[arg(help = "Path to the file to blame")]
        path: String,
        #[arg(long, help = "Blame the file as it was right after this change id")]
        at: Option<String>,
    },
    /// Show why a specific line was changed and its history
    #[command(after_help = "Examples:\n  trail why src/main.rs:42")]
    Why {
        #[arg(help = "<path>:<line> to inspect")]
        target: String,
    },
    /// Show unified diff from an earlier state to now
    Diff {
        #[arg(long, allow_hyphen_values = true, help = "Time or duration (e.g. 1h, 30s)")]
        since: Option<String>,
        #[arg(help = "Optional file path")]
        path: Option<String>,
    },
    /// Undo a specific change or all changes by an actor in a time window
    #[command(
        after_help = "Examples:\n  trail undo <change id>\n  trail undo --actor claude --since 1h --dry-run\n  trail undo --intent <intent id> --dry-run"
    )]
    Undo {
        #[arg(help = "The change ID to undo")]
        id: Option<String>,
        #[arg(long, help = "Revert changes made by this actor")]
        actor: Option<String>,
        #[arg(long, help = "Revert every change recorded under this intent id (a whole task)")]
        intent: Option<String>,
        #[arg(long, help = "Revert every change of this session id")]
        session: Option<String>,
        #[arg(long, help = "Revert every change of this command id (a bulk change is one command)")]
        command: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Time window (e.g. 1h, 30m)")]
        since: Option<String>,
        #[arg(long, help = "Print what would be done without modifying files or database")]
        dry_run: bool,
        #[arg(long, help = "Strict mode: refuse if file was edited since")]
        strict: bool,
    },
    /// Redo an undone change
    Redo {
        #[arg(help = "The undo change ID to redo")]
        id: String,
        #[arg(long, help = "Print what would be done without modifying files or database")]
        dry_run: bool,
    },
    /// Restore a file to its state at a change ID or time
    #[command(after_help = "Examples:\n  trail restore a.txt --at <change id>")]
    Restore {
        #[arg(help = "The file path to restore")]
        path: String,
        #[arg(long, help = "Change ID or timestamp")]
        at: String,
    },
    /// Pack loose objects into delta packs (garbage collection)
    Gc,
    /// Verify database hash chain integrity
    Verify,
    /// Add paths to .trailignore so Tracewise stops recording them
    #[command(after_help = "Examples:\n  trail ignore 'dist/' '*.log'")]
    Ignore {
        #[arg(required = true, help = "Ignore patterns (same syntax as .gitignore)")]
        patterns: Vec<String>,
    },
    /// Remove the history of ignored (or chosen) paths and keep the chain verifiable
    #[command(
        after_help = "Examples:\n  trail prune --dry-run\n  trail prune --yes\n  trail prune --path 'build/**' --older-than 90d --yes"
    )]
    Prune {
        #[arg(long = "path", help = "Prune this path glob instead of the ignored paths (repeatable)")]
        paths: Vec<String>,
        #[arg(long, help = "Only records older than this (e.g. 90d, 12w)")]
        older_than: Option<String>,
        #[arg(long, help = "Print what would be removed without changing anything")]
        dry_run: bool,
        #[arg(long, help = "Do not ask for confirmation (required without a terminal)")]
        yes: bool,
        #[arg(long, help = "Write a copy of the database to this file first")]
        backup: Option<String>,
    },
    /// Watcher daemon management
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Hook called by agent harness
    Hook {
        /// Hook event name
        #[arg(default_value = "")]
        event: String,
        #[arg(long, help = "Agent adapter name")]
        from: Option<String>,
        #[arg(long, help = "Record raw hook payload JSON into <dir>")]
        record: Option<PathBuf>,
    },
    /// Agent adapters and configuration
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Terminal shell integration
    Shell {
        #[command(subcommand)]
        action: ShellAction,
    },
    /// List recorded commands
    #[command(name = "commands")]
    CommandHistory,
    /// Git wrapper management
    #[command(name = "git-wrapper")]
    GitWrapper {
        #[command(subcommand)]
        action: GitWrapperAction,
    },
    /// Git hooks for git apps that do not run the Tracewise git wrapper
    #[command(name = "git-hooks")]
    GitHooks {
        #[command(subcommand)]
        action: GitHooksAction,
    },
    /// Called by the hooks that `git-hooks install` writes
    #[command(name = "git-hook")]
    GitHook {
        /// Git hook name (pre-commit, post-commit, post-checkout, post-merge, post-rewrite)
        name: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage intents (reasons)
    #[command(after_help = "Examples:\n  trail intent add --why \"fix the title\" a.txt")]
    Intent {
        #[command(subcommand)]
        action: IntentAction,
    },
    /// Add a reason to an existing change
    #[command(after_help = "Examples:\n  trail note --last \"why these changed\"\n  trail note <change id> \"why\"")]
    Note {
        #[arg(long, help = "Add note to every pending change of the current actor")]
        last: Option<String>,
        #[arg(help = "The change ID (ULID)")]
        change_id: Option<String>,
        #[arg(help = "The reason text")]
        why: Option<String>,
    },
    /// The opt-in sensor that names the process behind every write
    Strict {
        #[command(subcommand)]
        action: StrictAction,
    },
    /// Give a copied repository folder its own id and device name
    AdoptCopy,
    /// List work that agents started in the background
    Ps,
    /// Manage background work
    Background {
        #[command(subcommand)]
        action: BackgroundAction,
    },
    /// Manage locks
    #[command(after_help = "Examples:\n  trail lock list\n  trail lock release a.txt")]
    Lock {
        #[command(subcommand)]
        action: LockAction,
    },
    /// Manage sharing configuration
    Share {
        #[command(subcommand)]
        action: ShareAction,
    },
    /// Push unpushed records to companion remote
    Push,
    /// Fetch records from companion remote
    Fetch,
    /// Rebuild database from local batches
    Rebuild,
    /// Read or change Tracewise's settings (retention, profile, reasons mode, storage, durability)
    #[command(
        after_help = "Examples:\n  trail config get\n  trail config get reasons.mode\n  trail config set reasons.mode enforce\n  trail config set --global retention.commands 14d"
    )]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// List the repositories the daemon watches, with change counts and sizes
    Repos,
    /// Inspect process hierarchy and identity
    Proc {
        #[command(subcommand)]
        action: ProcAction,
    },
    /// Run a command under a declared reason intent
    #[command(after_help = "Examples:\n  trail run --why \"run the tests\" -- cargo test")]
    Run {
        #[arg(long, help = "Why this command is being run")]
        why: String,
        #[arg(last = true, required = true, help = "Command and arguments to execute")]
        command: Vec<String>,
    },
    /// List changes without a reason
    Pending,
    /// Manage command shims for external tools
    Shims {
        #[command(subcommand)]
        action: ShimsAction,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum GitHooksAction {
    /// Write the five hooks into this repository (an existing hook is kept and still runs)
    Install,
    /// Remove them and put back the hooks that were kept
    Uninstall,
    /// Show how each hook stands
    Status,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ShimsAction {
    /// Install command shims into a directory
    Install {
        #[arg(long, help = "Directory to install shims into")]
        dir: PathBuf,
    },
    /// Uninstall command shims from a directory
    Uninstall {
        #[arg(long, help = "Directory to uninstall shims from")]
        dir: PathBuf,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProcAction {
    /// Show process chain up to pid 1
    Chain {
        #[arg(long, help = "Process ID (default: parent PID)")]
        pid: Option<u32>,
    },
    /// Determine who is acting from the process hierarchy
    Who {
        #[arg(long, help = "Process ID (default: parent PID)")]
        pid: Option<u32>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    /// Show one setting, or all of them, with where each value comes from
    Get { key: Option<String> },
    /// Change a setting in .trailconfig (or the global file with --global)
    Set {
        key: String,
        value: String,
        #[arg(long, help = "Write the global file ($TRAIL_HOME/config.toml) instead of this repository's .trailconfig")]
        global: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum KeyAction {
    /// Print the key as one string to hand to a teammate or keep as a backup (it is the key itself: a secret)
    Export {
        #[arg(long, help = "Key id to export (default: the current key)")]
        id: Option<String>,
    },
    /// Add an exported key to this clone
    Import {
        key: String,
        #[arg(long, help = "Use it for new batches too (after a rotation); otherwise only to read")]
        current: bool,
    },
    /// List key ids (never the keys) and which is current
    List,
    /// Make a new key for new batches; old keys stay so old batches can still be read
    Rotate,
    /// Rotate, then push every record of this device again under the new key (for a lost or leaked key; this device's data must be intact)
    Rekey {
        #[arg(long, help = "Also delete this device's old ref on the remote first (the old batches become unreachable there)")]
        prune_old: bool,
        #[arg(long, help = "Confirm --prune-old")]
        yes: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ShareAction {
    /// Initialize sharing with companion remote
    Init {
        #[arg(long, help = "URL of the companion git repository (default: this project's own origin, with hidden refs)")]
        remote: Option<String>,
        #[arg(long, help = "Sharing level (full, summary, off); default summary")]
        level: Option<String>,
        #[arg(long, help = "Keep the batches under refs/trail/device/ (no branches on the remote); the default when --remote is left out")]
        hidden: bool,
        #[arg(long, help = "Encrypt every batch with a project key kept on this machine only (made now if there is none)")]
        encrypt: bool,
    },
    /// Approve the remote named in .trailconfig so `trail push` may upload to it
    Trust,
    /// Show how sharing is set up here: remote, level, refs, encryption, key, warnings
    Status,
    /// The project's encryption key: export, import, list, rotate, rekey
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// List the signing keys pinned for each device; `--forget DEVICE` drops one after a deliberate key change
    Keys {
        #[arg(long, help = "Device id whose pinned key to forget")]
        forget: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum LockAction {
    /// List active locks
    List,
    /// Release a lock on a path or all locks
    Release {
        #[arg(help = "Path to release")]
        path: Option<String>,
        #[arg(long, help = "Release every lock")]
        all: bool,
    },
    /// Wait for a lock on a path
    Wait {
        #[arg(help = "Path to wait for")]
        path: String,
        #[arg(long, help = "Agent name")]
        agent: Option<String>,
        #[arg(long, help = "Session ID")]
        session: Option<String>,
        #[arg(long, default_value = "30", help = "Timeout in seconds")]
        timeout: u64,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum IntentAction {
    /// Add an intent
    Add {
        #[arg(long, help = "Why this change is being made")]
        why: String,
        #[arg(long, help = "Task ID if any")]
        task: Option<String>,
        #[arg(long, help = "Agent name")]
        agent: Option<String>,
        #[arg(long, help = "Session ID")]
        session: Option<String>,
        #[arg(help = "Paths or globs covered by this intent")]
        paths: Vec<String>,
    },
    /// List active intents
    List {
        #[arg(long, help = "Filter by session ID")]
        session: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ShellAction {
    /// Initialize shell integration
    Init {
        shell: String,
        #[arg(long = "non-interactive", help = "Print non-interactive shell hook block")]
        non_interactive: bool,
    },
    /// Pre-execution hook for shell command
    Preexec {
        #[arg(long)]
        id: String,
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// Pre-command hook after shell command finishes
    Precmd {
        #[arg(long)]
        id: String,
        #[arg(long)]
        exit: i32,
    },
    /// Begin a command window for shell command
    Begin {
        #[arg(long)]
        pid: u32,
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// End a command window for shell command
    End {
        #[arg(long)]
        pid: u32,
        #[arg(long)]
        exit: i32,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum GitWrapperAction {
    /// Install git wrapper
    Install {
        #[arg(long)]
        dir: PathBuf,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum StrictAction {
    /// Is the sensor installed, connected and running, and if not, why
    Status,
    /// Install the sensor (needs sudo; run it yourself)
    Install {
        /// Print what would be run instead of running it
        #[arg(long)]
        dry_run: bool,
        /// Event preset: light (default) or full (adds write events, about four times the cost)
        #[arg(long, default_value = "light")]
        events: String,
    },
    /// Remove the sensor
    Uninstall {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum BackgroundAction {
    /// Stop attributing later writes to a background window (see `trail ps`)
    Stop { id: String },
}

#[derive(Subcommand, Debug, Clone)]
pub enum DaemonAction {
    /// Run daemon in foreground
    Run,
    /// Record the changed paths named on stdin (one JSON line each); started by trail-watchd
    #[command(hide = true)]
    ScanPaths,
    /// Print 1 when an agent is working or a background job is alive (the strict sensor runs only then)
    #[command(hide = true)]
    Active,
    /// Release expired locks and repair lock flags; started by trail-watchd
    #[command(hide = true)]
    Sweep,
    Start,
    Stop,
    Status,
    /// Install daemon LaunchAgent plist
    Install {
        #[arg(long = "no-load", help = "Do not load into service manager")]
        no_load: bool,
    },
    /// Uninstall daemon LaunchAgent plist
    Uninstall,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AgentAction {
    /// Install agent hooks and configuration
    Install {
        /// Agent name (claude, codex, gemini, antigravity, cursor)
        name: String,
        #[arg(long, default_value = "project", help = "Configuration scope (project, user)")]
        scope: String,
    },
    /// Uninstall agent hooks and configuration
    Uninstall {
        /// Agent name (claude, codex, gemini, antigravity, cursor)
        name: String,
    },
    /// Status of agent adapters
    Status,
    /// List the built-in agents and the adapter files (user and project)
    List,
    /// Show what a payload would become with an adapter file, without running the agent
    Check {
        /// The adapter file (`<name>.toml`)
        file: std::path::PathBuf,
        /// A hook payload (JSON file)
        payload: std::path::PathBuf,
    },
}

#[cfg(test)]
#[path = "test_cli.rs"]
mod tests;
