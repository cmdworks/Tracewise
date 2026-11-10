#!/bin/sh
# Installs Tracewise for this user: downloads a release, checks it, and runs `tracewise setup`
# (programs, command wrappers, git wrapper, shell hook, watcher), then optional shell bindings and agent hooks.
#
#   curl -fsSL https://raw.githubusercontent.com/cmdworks/tracewise/main/scripts/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --fish --login yes
#   cd my-project && curl -fsSL .../install.sh | sh -s -- --agents claude,codex
#
# Run with --help for every option. Needs no sudo (except --strict).
set -eu

REPO="cmdworks/tracewise"
VERSION=""
ARCHIVE=""
SHELL_RC=1
FISH=0
AGENTS=""
LOGIN="ask"
PROFILE="core"
START=1
STRICT=0
UNINSTALL=0

usage() {
  cat <<EOF
Install Tracewise.

Usage: install.sh [options]

  --version <tag>        release to install, e.g. v0.1.0 (default: the newest, pre-releases included)
  --archive <file>       install from a downloaded tracewise-*.tar.gz instead of GitHub
  --fish                 also bind fish (done anyway when fish is your shell or ~/.config/fish exists)
  --no-shell             leave shell files alone: no PATH and no terminal hook (zsh, bash, fish)
  --agents <list>        hooks for coding agents in the git repository you run this from:
                         claude, codex, cursor, gemini, antigravity (comma separated)
  --login <yes|no|ask>   start the watcher at login (default: ask; no when not run in a terminal)
  --no-start             do not start the watcher now
  --profile <core|full>  default profile (default: core)
  --strict               also install the strict sensor (asks for sudo)
  --uninstall            remove what an earlier install added (project histories stay)
  -h, --help             this help

Environment: GITHUB_TOKEN is used for downloads when set.
EOF
}

say() { printf '\033[1;38;5;166m==>\033[0m %s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "needs '$1'"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION=${2:?--version needs a tag}; shift 2 ;;
    --archive) ARCHIVE=${2:?--archive needs a file}; shift 2 ;;
    --fish) FISH=1; shift ;;
    --no-shell) SHELL_RC=0; shift ;;
    --agents) AGENTS=${2:?--agents needs a list}; shift 2 ;;
    --login) LOGIN=${2:?--login needs yes, no or ask}; shift 2 ;;
    --no-start) START=0; shift ;;
    --profile) PROFILE=${2:?--profile needs core or full}; shift 2 ;;
    --strict) STRICT=1; shift ;;
    --uninstall) UNINSTALL=1; shift ;;
    -h | --help) usage; exit 0 ;;
    *) usage >&2; die "unknown option: $1" ;;
  esac
done

case "$LOGIN" in yes | no | ask) ;; *) die "--login must be yes, no or ask" ;; esac
case "$PROFILE" in core | full) ;; *) die "--profile must be core or full" ;; esac
# setup can only ask when someone can answer (curl | sh has no terminal on stdin)
if [ "$LOGIN" = ask ] && [ ! -t 0 ]; then LOGIN=no; fi

TRACEWISE_HOME=${TRACEWISE_HOME:-$HOME/.tracewise}
for agent in $(echo "$AGENTS" | tr ',' ' '); do
  case "$agent" in claude | codex | cursor | gemini | antigravity) ;; *) die "unknown agent '$agent'" ;; esac
done
if [ -n "$AGENTS" ] && ! git rev-parse --show-toplevel >/dev/null 2>&1; then
  die "--agents installs hooks into a project: run this from inside a git repository"
fi

if [ "$UNINSTALL" = 1 ]; then
  if command -v tracewise >/dev/null 2>&1; then tracewise uninstall; elif [ -x "$TRACEWISE_HOME/bin/tracewise" ]; then "$TRACEWISE_HOME/bin/tracewise" uninstall; fi
  say "Tracewise removed. Each project's history stays in its .git/tracewise folder."
  exit 0
fi

[ "$(uname -s)" = Darwin ] || die "Tracewise releases are for macOS for now; on Linux build from source (cargo build --release)"
need tar
need shasum
case "$(uname -m)" in
  arm64 | aarch64) ARCH=arm64 ;;
  x86_64) ARCH=x86_64 ;;
  *) die "unsupported CPU: $(uname -m)" ;;
esac

WORK=$(mktemp -d "${TMPDIR:-/tmp}/tracewise-install.XXXXXX")
trap 'rm -rf "$WORK"' EXIT INT TERM

if [ -z "$ARCHIVE" ]; then
  need curl
  auth=""
  [ -n "${GITHUB_TOKEN:-}" ] && auth="Authorization: Bearer $GITHUB_TOKEN"
  api() { curl -fsSL ${auth:+-H "$auth"} -H "Accept: $2" "https://api.github.com/repos/$REPO/$1"; }
  if [ -z "$VERSION" ]; then
    VERSION=$(api "releases?per_page=1" application/vnd.github+json | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
    [ -n "$VERSION" ] || die "no release found for $REPO"
  fi
  name="tracewise-${VERSION#v}-macos-$ARCH.tar.gz"
  say "Downloading Tracewise $VERSION for $ARCH"
  release=$(api "releases/tags/$VERSION" application/vnd.github+json) || die "release $VERSION not found"
  for file in "$name" "$name.sha256"; do
    # the asset's API url (with its id) comes before its name in the same object; this works for private repositories too
    id=$(printf '%s' "$release" | F="$file" perl -0ne 'if (/"url":\s*"[^"]*\/assets\/(\d+)"[^{}]*?"name":\s*"\Q$ENV{F}\E"/) { print $1 }')
    [ -n "$id" ] || die "$file is not part of release $VERSION"
    api "releases/assets/$id" application/octet-stream >"$WORK/$file"
  done
  (cd "$WORK" && shasum -a 256 -c "$name.sha256" >/dev/null) || die "checksum mismatch for $name"
  ARCHIVE="$WORK/$name"
fi

[ -f "$ARCHIVE" ] || die "no such archive: $ARCHIVE"
tar -xzf "$ARCHIVE" -C "$WORK"
DIR=$(find "$WORK" -mindepth 1 -maxdepth 1 -type d -name 'tracewise-*' | head -1)
[ -n "$DIR" ] && [ -x "$DIR/tracewise" ] || die "the archive holds no tracewise program"
(cd "$DIR" && shasum -a 256 -c SHA256SUMS >/dev/null) || die "a program in the archive does not match SHA256SUMS"
# releases are not notarized yet; the download flag would make macOS refuse to run them
xattr -dr com.apple.quarantine "$DIR" 2>/dev/null || true

# Tracewise used to be called Trail; its wrappers, shell blocks and watcher would run beside the new ones
if [ -x "$HOME/.trail/bin/trail" ]; then
  say "Removing the old Trail install (its project histories in .git/trail stay)"
  "$HOME/.trail/bin/trail" uninstall >/dev/null 2>&1 || say "could not remove it; run: ~/.trail/bin/trail uninstall"
fi

say "Installing programs, command wrappers, git wrapper and watcher"
set -- --login "$LOGIN" --profile "$PROFILE"
[ "$START" = 0 ] && set -- "$@" --no-start
[ "$STRICT" = 1 ] && set -- "$@" --strict
[ "$SHELL_RC" = 0 ] && set -- "$@" --no-shell-rc
# setup binds fish when fish is the login shell or its config folder exists
[ "$FISH" = 1 ] && [ "$SHELL_RC" = 1 ] && mkdir -p "$HOME/.config/fish"
"$DIR/tracewise" setup "$@"

BIN="$TRACEWISE_HOME/bin"
if [ -n "$AGENTS" ]; then
  "$BIN/tracewise" init >/dev/null 2>&1 || true
  for agent in $(echo "$AGENTS" | tr ',' ' '); do
    "$BIN/tracewise" agent install "$agent" >/dev/null && say "hooks for $agent installed in $(git rev-parse --show-toplevel)"
  done
fi

say "Done. Open a new terminal, then run: tracewise doctor --all"
say "In a project: tracewise init"
