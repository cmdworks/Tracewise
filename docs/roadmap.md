# Roadmap

What is planned, roughly in this order. Nothing here is a promise or a date; shipped work is in the [changelog](./changelog).

## Next

- **Agent skill from `init`.** `tw init` finds the agents a project uses and offers to connect them: a valid Claude Code skill (`.claude/skills/tracewise/SKILL.md`, with the required header) and a short `AGENTS.md` block for Codex, Cursor and Gemini. Files stay local by default (`.git/info/exclude`); `--commit-files` shares them with a team. The skill and the hook messages use `tw` to save tokens, and `tracewise doctor` checks that `tw` is ours.
- **Command reference.** One page per command group with options, examples and the `--json` shape, checked against `--help` in CI so it cannot drift.
- **Guides.** One page per agent (Claude Code, Codex, Cursor, Gemini, Antigravity), a team guide (locks, sharing, CI reports) and troubleshooting driven by `doctor` messages.

## Later

- **Signed and notarized macOS builds**, then a Homebrew cask.
- **The desktop app bundles the command line**, so a disk image alone is a complete install.
- **Linux**, then **Windows**. Several crates still use Unix-only APIs; Linux work is on a branch.
- **`agent install --scope user`**, so hooks can be installed for every project at once.
