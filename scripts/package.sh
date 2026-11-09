#!/bin/bash
# Builds the macOS release archive: target/dist/tracewise-<version>-macos-<arch>.tar.gz and its .sha256.
# Unsigned and not notarized. Usage: scripts/package.sh [rust-target], e.g. aarch64-apple-darwin.
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo "package.sh: macOS only" >&2
  exit 2
fi
target=${1:-}
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "tracewise"))')
if [ -n "$target" ]; then
  cargo build --locked --release --workspace --target "$target"
  bin_dir="target/$target/release"
  arch=${target%%-*}
else
  cargo build --locked --release --workspace
  bin_dir=target/release
  arch=$(uname -m)
fi
[ "$arch" = aarch64 ] && arch=arm64
name="tracewise-$version-macos-$arch"
out=target/dist
rm -rf "${out:?}/$name" && mkdir -p "$out/$name"
# the list setup installs and doctor checks (setup.rs PROGRAMS); `tracewise` is shipped to run setup, and setup links it to `trail`
programs="trail trail-git trail-shim trail-watchd trail-sensor"
for program in tracewise $programs; do
  cp "$bin_dir/$program" "$out/$name/"
done
(cd "$out/$name" && shasum -a 256 $programs > SHA256SUMS)
cp LICENSE README.md "$out/$name/"
cargo metadata --locked --format-version 1 | python3 scripts/third_party_notices.py > "$out/$name/THIRD-PARTY-NOTICES"
tar -czf "$out/$name.tar.gz" -C "$out" "$name"
(cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
echo "$out/$name.tar.gz"
