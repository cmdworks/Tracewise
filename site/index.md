---
layout: home
title: Tracewise
titleTemplate: A flight recorder for your code

hero:
  name: Tracewise
  text: A flight recorder for your code
  tagline: Who changed every line, when, and why — you, a coding agent, a script or git. Recorded. Explained. Reversible.
  image:
    src: /logo.svg
    alt: Tracewise logo
  actions:
    - theme: brand
      text: Get started
      link: /getting-started
    - theme: alt
      text: Install
      link: "#install"
    - theme: alt
      text: View on GitHub
      link: https://github.com/cmdworks/Tracewise

features:
  - icon: 🔍
    title: Every change, attributed
    details: Each file change is recorded with its author (you, Claude Code, Codex, Cursor, Gemini, Antigravity, a script or git), how sure Tracewise is, and a reason.
  - icon: ↩️
    title: Undo by line, not by file
    details: Take back one change, or everything an agent did in the last hour. Only the lines it wrote go; your later edits stay.
  - icon: 🔒
    title: Locks between agents
    details: Optional locks so two agents never edit the same file at once. Refused edits are recorded too.
  - icon: 🧾
    title: Tamper-evident history
    details: A hash-chained record kept inside .git. Your git history is never touched, and `verify` proves nothing was altered.
  - icon: 🤝
    title: Share between machines
    details: Opt-in sharing of history with signed, optionally encrypted batches.
  - icon: 🖥️
    title: CLI and desktop app
    details: Everything from the terminal, or a live timeline, blame and undo in the desktop app.
---

<div class="home-extra">

## See it work {#demo}

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

## Install {#install}

macOS (Apple silicon and Intel). The installer downloads the newest release, checks it, and sets up the programs, command wrappers, git wrapper, shell hook and watcher. It needs no `sudo`.

```bash
curl -fsSL https://raw.githubusercontent.com/cmdworks/Tracewise/main/scripts/install.sh | sh
```

Then, in a new terminal:

```bash
tracewise doctor --all   # check the installation
cd your-project
tracewise init           # start recording this repository
```

More ways to install, options, and the desktop app are in [Getting started](./getting-started) and the [Desktop app](./desktop-app) page. Linux and Windows are planned.

</div>
