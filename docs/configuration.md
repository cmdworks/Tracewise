# Configuration

Tracewise works without any configuration. This page covers what you can change.

## Where settings live

Settings are read from, strongest first:

1. environment variables (yours only — agents cannot use them to loosen anything);
2. the project file `.trailconfig` in the repository root (commit it to share settings with your team);
3. the global file `~/.trail/config.toml`;
4. built-in defaults.

Both files use the same keys. The easiest way to read and change them:

```bash
tracewise config get                         # every setting and where its value comes from
tracewise config get reasons.mode
tracewise config set reasons.mode enforce    # in .trailconfig
tracewise config set --global profile.name full
```

A file that does not parse is ignored, and `tracewise doctor` says so. Agents cannot edit either file.

## Profiles

A profile switches groups of features on. `core` is the default.

| Profile | Adds |
|---|---|
| `core` | the change log, `log` / `blame` / `why` / `show` / `diff`, reasons, `undo` / `redo` / `restore`, `verify`, agent hooks, watcher, shell hook, git wrapper |
| `coordination` | [locks](how-it-works.md#7-locks), the grace period after your saves, notices to agents about others' changes |
| `policy` | protected files, the commit check (no commit while agent changes lack a reason), and refusals when an agent tries to pose as a person |
| `strict` | the optional sensor that names the exact process behind every write (needs `sudo` and Full Disk Access) |
| `share` | [sharing](sharing.md) through a git remote |

```toml
[profile]
name = "core"          # core, or full for everything

# or switch single features on:
coordination = true
share = true
```

## Reasons

```toml
[reasons]
mode = "warn"          # off | warn | enforce
```

| Mode | An agent editing without a reason… |
|---|---|
| `off` | is not asked |
| `warn` *(default)* | may edit; the change is listed by `tracewise pending` |
| `enforce` | is denied, told how to give a reason, and cannot end its turn while changes lack one |

A reason must be at least 12 characters and not a placeholder. People are never forced.

## Storage

```toml
[storage]
max_file = "5mb"       # larger files are recorded by hash only
secrets  = "hash"      # a text file holding a known credential is recorded by hash only; "store" keeps it

[durability]
level = "full"         # full: every record is on disk before it is reported; normal: slightly faster, a power cut may lose the newest records

[retention]
commands = "30d"       # how long recorded command lines are kept ("off" keeps them all); changes are never pruned
```

## Ignoring files

Tracewise never records a path that git ignores (every `.gitignore`, `.git/info/exclude`, your global git ignore) or that is OS clutter such as `.DS_Store`. To ignore more without touching `.gitignore`, add patterns to `.trailignore` (same syntax):

```bash
tracewise ignore 'dist/' '*.log'
```

To remove history that was recorded before a path was ignored:

```bash
tracewise prune --dry-run
tracewise prune
```

`prune` keeps the history verifiable: `tracewise verify` still passes afterwards.

## Adding an agent

Claude Code, Codex, Cursor, Gemini and Antigravity are built in. Any other agent with a hook system can be added without rebuilding Tracewise: put a small adapter file `<name>.toml` in `.trail/adapters/` of the project or in `~/.trail/adapters/`. It says where the event name, session, tool and file path sit in the agent's hook payload, and how to answer. Adapter files are data and are never executed.

```bash
tracewise agent list                                 # built-in agents and adapter files found
tracewise agent check myagent.toml payload.json      # what a sample payload would become
tracewise agent install myagent
```

## Environment variables

| Variable | Meaning |
|---|---|
| `TRAIL_HOME` | the global folder (default `~/.trail`) |
| `TRAIL_PROFILE` | `core` or `full`, over the files |
| `TRAIL_REASONS` | reasons mode for this shell |
| `TRAIL_WHY` | the reason for the next command |
| `TRAIL_ACTOR` | who is acting, e.g. `script:nightly-build` |
| `TRAIL_LOCK_TTL_SECS` | lock lifetime (default 120) |
| `TRAIL_EDIT_GRACE_SECS` | how long agents wait after your save (default 20) |

The variables keep the `TRAIL_` prefix of Tracewise's earlier name.
