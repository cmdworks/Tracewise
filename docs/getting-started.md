# Getting started

This guide takes you from nothing to a project whose every change is recorded, explained and reversible. It takes about five minutes.

## 1. Install

Tracewise runs on macOS (Apple silicon and Intel).

**From a release:**

```bash
tar -xzf tracewise-0.1.0-macos-arm64.tar.gz    # or -x86_64 on an Intel Mac
cd tracewise-0.1.0-macos-arm64
./tracewise setup
```

**From source** (needs a current stable Rust from [rustup](https://rustup.rs)):

```bash
git clone https://github.com/cmdworks/Tracewise
cd Tracewise
cargo build --release
./target/release/tracewise setup
```

`setup` asks whether the watcher should start at login, then:

- copies the programs to `~/.tracewise/bin`;
- adds a `git` wrapper and a few command wrappers (`sed`, `mv`, `cp`, `rm`, `tee`) there, so writes made through them are attributed;
- adds one clearly marked block to `~/.zshenv` and `~/.zshrc` (your other lines are kept; a backup is written);
- starts the background watcher.

It never asks for `sudo`. Open a new terminal and check that everything is in place:

```bash
tracewise doctor --all
```

<a id="unsigned-binaries"></a>
> **Unsigned binaries.** Release binaries are not yet signed by Apple. If macOS refuses to run them, remove the download flag once: `xattr -dr com.apple.quarantine tracewise-*`.

## 2. Start recording a project

Tracewise works in any git repository:

```bash
cd ~/code/my-project
tracewise init
```

`init` records the current state of every tracked file as the *baseline* and registers the repository with the watcher. From now on every change is recorded, whoever makes it. Nothing is added to your working tree: the history lives in `.git/tracewise/`, which git never shows or commits.

## 3. Connect your coding agents

Agents with hooks tell Tracewise exactly which file they are about to edit, in which session, and why. Install the hooks for the agents you use, in the project:

```bash
tracewise agent install claude       # Claude Code
tracewise agent install codex        # also: cursor, gemini, antigravity
```

Use `--scope user` to install them for every project. Agents without hooks are still recorded by the watcher, with less certainty about who made the change. Other agents can be added with a small adapter file — see [Configuration](configuration.md#adding-an-agent).

## 4. See what happened

```bash
tracewise log                        # newest changes first
tracewise log --actor claude --since 2h
tracewise log --group                # changes grouped by task
tracewise show <change id>           # one change with its diff
tracewise blame src/app.rs           # who last changed each line, and why
tracewise why src/app.rs:42          # the full story of one line
tracewise diff src/app.rs --since 1h # what a file looked like an hour ago, as a diff
```

Every command accepts `--json` for scripts and tools.

## 5. Say why

Agents declare a reason before editing; Tracewise can require it (see [reasons](configuration.md#reasons)). Agents and people can also do it by hand:

```bash
tracewise intent add --why "fix the empty-input crash" src/parser.rs
tracewise run --why "update dependencies" -- cargo update
tracewise pending                    # changes that have no reason yet
tracewise note --last "rename the config keys"
```

For people this is never required. When you commit, the commit message becomes the reason of your changes that have none.

## 6. Go back

Undo works on lines, not whole files: only the lines a change wrote are taken back, and later edits by others stay.

```bash
tracewise undo <change id>                     # one change
tracewise undo --actor codex --since 30m       # everything an agent did in the last half hour
tracewise undo --intent <intent id>            # one whole task
tracewise undo <change id> --dry-run           # show what would happen
tracewise redo <undo id>                       # changed your mind
tracewise restore src/app.rs --at <change id>  # put a whole file back as it was
```

An undo is itself recorded, so nothing is ever lost.

## 7. Keep it healthy

```bash
tracewise status          # this repository: changes, locks, watcher
tracewise doctor          # checks this repository; --all checks the installation
tracewise verify          # checks that no record was altered or removed
tracewise gc              # packs stored versions; run now and then
tracewise ignore build/   # stop recording a path (writes .tracewiseignore)
```

## Removing Tracewise

```bash
tracewise uninstall
```

This removes exactly what `setup` installed. The history of each project stays in its `.git/tracewise/` folder; delete that folder to remove it too.

## Next

- [How it works](how-it-works.md) — what happens behind each of these commands.
- [Commands](commands.md) — the full list.
- [Sharing](sharing.md) — the same history on every machine of your team.
