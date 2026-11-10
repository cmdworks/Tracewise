# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-11-10

### Changed
- License changed to GNU GPL v3 or later: changed versions that are distributed must stay open source under the same license.
- A release is built only from a `vX.Y.Z` tag that is higher than the newest existing release and matches the version in `Cargo.toml`; normal CI does not run on tags.
- Everything is now named Tracewise; the old Trail names are gone (no migration):
  programs `tracewise`, `tracewise-git`, `tracewise-shim`, `tracewise-watchd`, `tracewise-sensor` (the `trail` command is removed);
  `~/.tracewise` instead of `~/.trail`; each project's history in `.git/tracewise` instead of `.git/trail`;
  `.tracewiseconfig` and `.tracewiseignore`; `TRACEWISE_*` environment variables; the launchd job `dev.tracewise.daemon`;
  shared history under `tracewise/device/<device>`.
- `scripts/install.sh` removes an old Trail install first. Histories recorded by Trail stay in `.git/trail` and are not read.

### Fixed
- `uninstall` no longer leaves `SHA256SUMS` behind in the programs folder.

## [0.1.0] - 2026-11-09

First public release, for macOS (Apple silicon and Intel). Builds are not notarized yet.

### Added
- Records every file change in a project with its author (you, Claude Code, Codex, Cursor, Gemini, Antigravity, a script or git), a confidence level and a reason, in a hash-chained history inside `.git`.
- Capture through agent hooks, zsh, bash, fish, PowerShell and nushell hooks, command wrappers (`sed`, `mv`, `cp`, `rm`, `tee`), a git wrapper and git hooks, and a background watcher.
- `log`, `show`, `file`, `blame` (with `--at`), `why`, `diff`, `report`, `pending` and `repos` to see who changed what and why.
- `undo` (one change, or by actor, session, task or command, with `--dry-run`), `redo` and `restore`.
- Optional locks so two agents never edit one file at once; refused edits are recorded.
- `config get/set`, profiles, `.tracewiseignore`, `ignore` and `prune`.
- Opt-in sharing of history between devices, with signed and optionally encrypted batches.
- `verify` to check the history, and `doctor` to check the installation.
- A desktop app (preview) with Timeline, Now, Files, Undo, Projects, Health and Settings.
- A GitHub Action (`ci/report`) that verifies the history and writes a report to the job summary.

[0.1.0]: https://github.com/cmdworks/Tracewise/releases/tag/v0.1.0
