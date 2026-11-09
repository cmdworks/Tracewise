# Commands

Every command accepts `--json` for scripts and tools, and `-C <dir>` to run as if started in another folder. `tracewise <command> --help` shows all options with examples.

Change ids are the 26-character ids shown by `log` (for example `01M6ZGHB9C46J2SNR4HTTJV60E`). Times accept durations like `30m`, `2h`, `7d`, or a date and time.

## Setup

| Command | |
|---|---|
| `tracewise setup` | install for this user: programs, wrappers, shell hook, watcher (no `sudo`) |
| `tracewise uninstall` | remove exactly what `setup` installed; histories stay |
| `tracewise init` | start recording the current repository |
| `tracewise doctor [--all]` | check this repository, or the whole installation |
| `tracewise status` | this repository at a glance |

## Agents

| Command | |
|---|---|
| `tracewise agent install <name> [--scope project\|user]` | install hooks for `claude`, `codex`, `cursor`, `gemini`, `antigravity` or an adapter file |
| `tracewise agent uninstall <name>` | remove them |
| `tracewise agent status` | which agents are connected here |
| `tracewise agent list` | built-in agents and adapter files |
| `tracewise ps` | work that agents started in the background |

## Looking at history

| Command | |
|---|---|
| `tracewise log [path]` | changes, newest first. Filters: `--actor`, `--since`, `--grep`, `--limit`; `--group` groups them by task |
| `tracewise show <id>` | one change with its diff |
| `tracewise blame <path> [--at <id>]` | who last changed each line, and why |
| `tracewise why <path>:<line>` | the full story of one line |
| `tracewise diff [path] --since <time>` | what changed since then, as a diff |
| `tracewise file <path>` | who holds a file and who changed it lately |
| `tracewise commands` | recorded commands |
| `tracewise report` | one task as Markdown, for a review or a hand-off |

## Reasons

| Command | |
|---|---|
| `tracewise intent add --why "<reason>" <paths…>` | declare why these files are about to change |
| `tracewise run --why "<reason>" -- <command>` | run a command under a reason |
| `tracewise pending` | changes without a reason |
| `tracewise note <id> "<reason>"` | add a reason to one change |
| `tracewise note --last "<reason>"` | add it to all of your changes that lack one |

## Going back

| Command | |
|---|---|
| `tracewise undo <id>` | take back one change, line by line |
| `tracewise undo --actor <who> --since <time>` | take back everything someone did in a time window |
| `tracewise undo --intent <id>` / `--session <id>` / `--command <id>` | take back a task, a session, or one bulk command |
| `tracewise redo <undo id>` | put back what an undo removed |
| `tracewise restore <path> --at <id or time>` | put a whole file back as it was |

`undo` and `redo` accept `--dry-run`; `undo --strict` refuses files edited since.

## Locks

Needs the `coordination` profile.

| Command | |
|---|---|
| `tracewise lock list` | current locks |
| `tracewise lock release <path>` / `--all` | release a lock (an agent can release only its own) |
| `tracewise lock wait <path>` | wait until a file is free |

## Sharing

| Command | |
|---|---|
| `tracewise share init [--remote URL] [--level summary\|full\|off] [--encrypt]` | turn sharing on for the project |
| `tracewise share trust` | approve the remote named in `.tracewiseconfig` |
| `tracewise share status` | how sharing is set up here |
| `tracewise push` / `tracewise fetch` | send / import history |
| `tracewise rebuild` | rebuild the database from the batches |
| `tracewise share key export\|import\|list\|rotate\|rekey` | manage the encryption key |
| `tracewise share keys [--forget <device>]` | the signing keys pinned for each machine |

## Maintenance

| Command | |
|---|---|
| `tracewise verify` | check that no record was altered or removed |
| `tracewise gc` | pack stored versions and clean up |
| `tracewise ignore <patterns…>` | stop recording paths (writes `.tracewiseignore`) |
| `tracewise prune [--dry-run]` | remove the history of ignored paths |
| `tracewise config get [key]` / `set [--global] <key> <value>` | read or change settings |
| `tracewise repos` | repositories the watcher follows, with sizes |
| `tracewise daemon start\|stop\|status` | the background watcher |
| `tracewise git-hooks install` | git hooks for git apps that bypass the wrapper |
| `tracewise strict …` | the optional exact-attribution sensor |
| `tracewise adopt-copy` | give a copied repository folder its own identity |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | success |
| `1` | the command failed, or a check (`verify`, `doctor`) found a problem |
| `2` | wrong usage |
