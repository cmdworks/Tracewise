#!/bin/sh
# Prepares a release: checks the version is higher than the last tag, bumps every version, writes the
# CHANGELOG section from Conventional Commits (git-cliff), commits and tags. Pushing the tag builds it.
#
#   scripts/release.sh 0.3.0
set -eu
die() { printf 'release.sh: %s\n' "$*" >&2; exit 1; }

v=${1:?usage: scripts/release.sh X.Y.Z}
case $v in [0-9]*.[0-9]*.[0-9]*) ;; *) die "version must look like 1.2.3" ;; esac
cd "$(git rev-parse --show-toplevel)"
[ "$(git branch --show-current)" = main ] || die "run this on main"
[ -z "$(git status --porcelain)" ] || die "the working tree is not clean"
git fetch -q --tags origin
latest=$(git tag -l 'v[0-9]*' | sort -V | tail -1 | sed 's/^v//')
top=$(printf '%s\n%s\n' "${latest:-0.0.0}" "$v" | sort -V | tail -1)
{ [ "$top" = "$v" ] && [ "$v" != "${latest:-}" ]; } || die "$v is not higher than the last release ${latest:-none}"

perl -0pi -e 's/^version = "[^"]*"/version = "'"$v"'"/m' Cargo.toml crates/tracewise-platform/Cargo.toml ui/src-tauri/Cargo.toml
perl -0pi -e 's/"version": "[^"]*"/"version": "'"$v"'"/' ui/package.json ui/src-tauri/tauri.conf.json
perl -pi -e 's#download/v[0-9.]+/tracewise-[0-9.]+-#download/v'"$v"'/tracewise-'"$v"'-#g' README.md
perl -pi -e 's#tracewise-[0-9.]+-macos-#tracewise-'"$v"'-macos-#g' docs/getting-started.md
cargo update -w --offline >/dev/null

# a hand-written section for this version wins; otherwise it comes from the commits since the last tag
if ! grep -q "^## \[$v\]" CHANGELOG.md; then
  command -v git-cliff >/dev/null || die "needs git-cliff (brew install git-cliff)"
  section=$(mktemp)
  git-cliff --unreleased --tag "v$v" --strip all >"$section"
  [ -s "$section" ] || die "no feat/fix/docs commits since v${latest:-the start}; nothing to release"
  printf '\n' >>"$section"
  awk -v f="$section" '!done && /^## \[/ { while ((getline l < f) > 0) print l; done = 1 } { print }' CHANGELOG.md >CHANGELOG.new
  mv CHANGELOG.new CHANGELOG.md
  rm -f "$section"
fi
grep -q "^\[$v\]:" CHANGELOG.md || printf '[%s]: https://github.com/cmdworks/Tracewise/releases/tag/v%s\n' "$v" "$v" >>CHANGELOG.md

git commit -qam "chore(release): v$v"
git tag -a "v$v" -m "Tracewise v$v"
echo "Ready. Publish with: git push origin main && git push origin v$v"
