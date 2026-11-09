# Tracewise

**A flight recorder for your code.** Tracewise records every file change in a project — who made it, when, and why — whether it was you, a coding agent, a script or a git command. You can see the story behind any line and step back to any point, without touching your git history.

[![CI](https://github.com/cmdworks/Tracewise/actions/workflows/ci.yml/badge.svg)](https://github.com/cmdworks/Tracewise/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/cmdworks/Tracewise)](https://github.com/cmdworks/Tracewise/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

```console
$ tracewise blame src/parser.rs
   1 01M6ZGP1763ZWNTNPSJSCEQVSB claude/s1  2026-11-09T06:42:12Z | fn parse(input: &str) -> Result<Ast> {   <- "handle empty input without panicking"
   2 01M6ZGP1763ZWNTNPSJSCEQVSB claude/s1  2026-11-09T06:42:12Z |     let tokens = lex(input)?;            <- "handle empty input without panicking"
   3 baseline                                                   |     build(tokens)   (git: you, 2026-11-09)
   4 baseline                                                   | }

$ tracewise why src/parser.rs:2
Line 2 in src/parser.rs changed by claude/s1 at 2026-11-09T06:42:12Z (01M6ZGP1763ZWNTNPSJSCEQVSB)
Why: handle empty input without panicking

$ tracewise undo --actor claude --since 1h     # take it all back; your own edits stay
```

## Why

Coding agents edit many files quickly, often several at once, and git only sees the result when someone commits. Tracewise fills the gap between commits:

- **Every change, attributed.** Each edit is recorded with its author (you, Claude Code, Codex, Cursor, Gemini, a script, git) and how sure Tracewise is about it.
- **Every change, explained.** Agents declare a reason before they edit; people can add one later. `why` shows it for any line.
- **Any step, reversible.** Undo one change, everything an agent did in the last hour, or one whole task — your own edits to the same files stay.
- **No two writers on one file.** Optional locks stop two agents (or an agent and you) from editing the same file at the same time.
- **Out of your way.** History lives inside `.git`, never in your commits or branches. Tracewise never blocks git, your shell or an agent when something goes wrong.

## Install

Tracewise runs on **macOS** (Apple silicon and Intel). Linux and Windows are planned.

**From a release** — download the archive for your Mac from [Releases](https://github.com/cmdworks/Tracewise/releases), then:

```bash
tar -xzf tracewise-*-macos-*.tar.gz && cd tracewise-*
./tracewise setup
```

Release builds are not notarized yet. If macOS blocks them, clear the download flag first: `xattr -dr com.apple.quarantine .`

**From source** — needs a current stable Rust from [rustup](https://rustup.rs):

```bash
git clone https://github.com/cmdworks/Tracewise && cd Tracewise
cargo build --release
./target/release/tracewise setup
```

`setup` installs the programs in `~/.trail/bin`, adds one block to `~/.zshrc` and starts the background watcher. It needs no `sudo`. Open a new terminal and check the installation:

```bash
tracewise doctor --all
```

## Quick start

```bash
cd your-project
tracewise init                      # start recording this repository
tracewise agent install claude      # optional: hooks for an agent (claude, codex, cursor, gemini, antigravity)

# ...work as usual; edit files, let agents edit files...

tracewise log                       # what changed, by whom
tracewise why src/app.rs:42         # why this line looks the way it does
tracewise undo <change id>          # take one change back
```

The full walkthrough is in [Getting started](docs/getting-started.md).

## Documentation

| | |
|---|---|
| [Getting started](docs/getting-started.md) | Install, first project, agents, everyday commands |
| [How it works](docs/how-it-works.md) | How changes are captured, attributed, stored and undone |
| [Commands](docs/commands.md) | Every command, grouped by task |
| [Configuration](docs/configuration.md) | Profiles, reasons, ignore rules, settings |
| [Sharing](docs/sharing.md) | Sharing history between machines, optionally encrypted |

## Status

Tracewise is young (0.x). The record format may still change between minor versions, and the macOS binaries are not yet signed — see [Getting started](docs/getting-started.md#unsigned-binaries). Bug reports and ideas are welcome in [Issues](https://github.com/cmdworks/Tracewise/issues).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Security issues: [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE)
