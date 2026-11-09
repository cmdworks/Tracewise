# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-11-09

First public release, for macOS (Apple silicon and Intel). Builds are not notarized yet.

### Added
- Records every file change in a project with its author (you, Claude Code, Codex, Cursor, Gemini, Antigravity, a script or git), a confidence level and a reason, in a hash-chained history inside `.git`.
- Capture through agent hooks, zsh, bash, fish, PowerShell and nushell hooks, command wrappers (`sed`, `mv`, `cp`, `rm`, `tee`), a git wrapper and git hooks, and a background watcher.
- `log`, `show`, `file`, `blame` (with `--at`), `why`, `diff`, `report`, `pending` and `repos` to see who changed what and why.
- `undo` (one change, or by actor, session, task or command, with `--dry-run`), `redo` and `restore`.
- Optional locks so two agents never edit one file at once; refused edits are recorded.
- `config get/set`, profiles, `.trailignore`, `ignore` and `prune`.
- Opt-in sharing of history between devices, with signed and optionally encrypted batches.
- `verify` to check the history, and `doctor` to check the installation.
- A desktop app (preview) with Timeline, Now, Files, Undo, Projects, Health and Settings.
- A GitHub Action (`ci/report`) that verifies the history and writes a report to the job summary.

[0.1.0]: https://github.com/cmdworks/Tracewise/releases/tag/v0.1.0
