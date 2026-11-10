# Desktop app (preview)

The desktop app shows what Tracewise records: a live timeline of changes, who is working now, file history line by line, undo, and the health of the installation. It holds no data of its own. Every read and every action runs the `tracewise` program, so the app and the command line always agree.

## Install

1. Download the `.dmg` for your Mac from the [latest release](https://github.com/cmdworks/Tracewise/releases/latest): `tracewise-desktop-<version>-macos-arm64.dmg` (Apple silicon) or `-x86_64.dmg` (Intel). A `.sha256` file sits next to each.
2. Open it and drag Tracewise to Applications.
3. The app is not signed by Apple yet, so macOS blocks the first start. Either right-click the app and choose **Open**, or clear the download flag once:

```bash
xattr -dr com.apple.quarantine /Applications/Tracewise.app
```

### The command line

The app needs the `tracewise` program. At start it looks in `~/.tracewise/bin`, then in `PATH`. When it finds none it shows a screen with:

- **Install the command line** — runs the release installer of the app's own version (the same as the [one-line install](README.md#download)): programs, command wrappers, shell hook and watcher in `~/.tracewise`. No password is asked.
- **Check again** — after you installed it yourself.
- The one-line command, to copy into a terminal instead.

If the installer fails, its last lines are shown on that screen.

## Using it

Pick a project (one that `tracewise init` has registered) in the sidebar; the app remembers the project, the tab and the open file.

| Tab | Key | What it shows |
|---|---|---|
| Timeline | `1` | Every change, newest first, with a diff inspector; search and filter by actor |
| Now | `2` | Who is working: agents, pending changes with no reason, locks, background commands |
| Files | `3` | One file line by line: who wrote each line and why |
| Undo | `4` | Undoes so far, with redo |
| Projects | `5` | The projects Tracewise watches and the daemon |
| Health | `6` | The installation check (`tracewise doctor`) |
| Settings | `7` | Configuration and sharing |

Other keys: `j`/`k` or the arrow keys move through changes, `/` searches, `u` undoes the selected change (after a preview), `b` opens its file, `i` toggles the inspector, `r` refreshes, `⌘B` hides the sidebar, `?` lists the keys.

Undo works on lines, not whole files, and every undo is itself recorded. See [Getting started](docs/getting-started.md#6-go-back).

## What the app may do

The app is a thin window on the command line, with limits that cannot be turned off from the page:

- It runs only a fixed list of subcommands (`status`, `log`, `show`, `undo`, `doctor` and so on). `setup`, `uninstall`, `init` and shell commands are not on it.
- It acts only on repositories Tracewise already knows, and refuses paths with `..` or links that lead elsewhere.
- Output and run time are bounded; a command over either is stopped.
- It runs as you: variables that mark an agent session are removed, so its actions are attributed to you.

The one exception is the install screen above, which runs the release installer only when you press the button.

## Build and develop

From `ui/` (needs Node 22, pnpm and a stable Rust):

```bash
pnpm install
pnpm tauri dev                  # the native app with hot reload
pnpm check && pnpm test         # type-check and unit tests
pnpm tauri build --bundles dmg  # a disk image in ui/src-tauri/target/release/bundle/dmg
```

The Rust side has its own tests, which need a built command line:

```bash
cargo build --release --workspace
TRACEWISE_BIN=$PWD/target/release/tracewise cargo test --manifest-path ui/src-tauri/Cargo.toml
```

To work on the page in an ordinary browser against real history, see [ui/README.md](ui/README.md).

## Releases

Pushing a version tag builds the command-line archives and one disk image per Mac type, and attaches them, with checksums, to the same release. The versions of the app and the command line move together (`scripts/release.sh`).

Signing and notarizing the disk image needs an Apple Developer ID and is not set up yet; until then the app is a preview and the first-start step above applies.
