<div align="center">

<img alt="" title="Tracewise" src="assets/logo.svg" width="128"/>

# Tracewise

<a href="#download"><img alt="Download" src="https://img.shields.io/badge/download-macOS-E06324?style=for-the-badge"/></a>
<a href="docs/getting-started.md"><img alt="Getting started" src="https://img.shields.io/badge/getting%20started-guide-1f2328?style=for-the-badge"/></a>
<a href="docs/commands.md"><img alt="Commands" src="https://img.shields.io/badge/commands-reference-1f2328?style=for-the-badge"/></a>
<a href="https://github.com/cmdworks/Tracewise/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/cmdworks/Tracewise/ci.yml?branch=main&style=for-the-badge&label=CI"/></a>

A flight recorder for your code.<br>
Who changed every line, when, and why — you, a coding agent, a script or git.<br>
Recorded. Explained. Reversible.

</div>

<br>

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

Coding agents edit many files quickly, often several at once, and git only sees the result when someone commits. Tracewise fills the gap between commits, without touching your git history.

## Download

<a href="https://github.com/cmdworks/Tracewise/releases"><img alt="Latest release" src="https://img.shields.io/github/v/release/cmdworks/Tracewise?include_prereleases&style=for-the-badge&color=E06324" height="28px"/></a>
<a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-1f2328?style=for-the-badge" height="28px"/></a>

<table>
  <tr>
    <td><b>macOS</b></td>
    <td>
      <a href="https://github.com/cmdworks/Tracewise/releases/download/v0.1.0/tracewise-0.1.0-macos-arm64.tar.gz">Apple silicon</a> |
      <a href="https://github.com/cmdworks/Tracewise/releases/download/v0.1.0/tracewise-0.1.0-macos-x86_64.tar.gz">Intel</a>
    </td>
  </tr>
  <tr>
    <td><b>Linux</b>, <b>Windows</b></td>
    <td>planned</td>
  </tr>
</table>

```bash
tar -xzf tracewise-*-macos-*.tar.gz && cd tracewise-*
./tracewise setup          # programs in ~/.trail/bin, one block in ~/.zshrc, the background watcher; no sudo
tracewise doctor --all     # in a new terminal: check the installation
```

> [!NOTE]
> Release builds are not notarized yet. If macOS blocks them, clear the download flag first: `xattr -dr com.apple.quarantine .`

**From source** (needs a current stable Rust from [rustup](https://rustup.rs)):

```bash
git clone https://github.com/cmdworks/Tracewise && cd Tracewise
cargo build --release
./target/release/tracewise setup
```

## Features

- 📝 records **every file change** in a project, with its author and how sure Tracewise is about it
- 🤖 knows **Claude Code, Codex, Cursor, Gemini and Antigravity** apart from you, a script or git
- 💬 keeps **the reason** for each change: agents declare it before they edit, people add it later
- 🔍 `blame` and `why` show **the story behind any line**, also as it was at any earlier change
- ⏪ **undo** one change, everything an agent did in the last hour, or one whole task, and **redo** it
- 🕰️ **restore** any file to how it was at any point
- 🔒 optional **locks** stop two agents, or an agent and you, from editing one file at once
- 🐚 captures edits from **agent hooks, zsh, bash, fish, PowerShell, nushell, git and git apps**, with a watcher for the rest
- ⛓️ a **hash-chained history** inside `.git`: `verify` finds any changed or missing record
- 🔄 optional **sharing between machines**, signed per device and optionally encrypted
- 🖥️ a **desktop app** (preview) with a live timeline, file history, undo and health
- 🧪 a **GitHub Action** that verifies the history and adds a report to the job summary
- ⚡ fast enough to forget: about **5 ms per agent edit**, **0% CPU** when idle
- ...and it **never blocks** git, your shell or an agent when something goes wrong

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

## Documentation

| | |
|---|---|
| [Getting started](docs/getting-started.md) | Install, first project, agents, everyday commands |
| [How it works](docs/how-it-works.md) | How changes are captured, attributed, stored and undone |
| [Commands](docs/commands.md) | Every command, grouped by task |
| [Configuration](docs/configuration.md) | Profiles, reasons, ignore rules, settings |
| [Sharing](docs/sharing.md) | Sharing history between machines, optionally encrypted |

## Status

Tracewise is young (0.x). The record format may still change between minor versions. Bug reports and ideas are welcome in [Issues](https://github.com/cmdworks/Tracewise/issues); please report security problems privately as described in [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE)
