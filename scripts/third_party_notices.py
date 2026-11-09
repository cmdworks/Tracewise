#!/usr/bin/env python3
"""Writes THIRD-PARTY-NOTICES for the Rust crates a release builds: name, version, license, and the license files each crate ships.
Usage: cargo metadata --locked --format-version 1 | scripts/third_party_notices.py > THIRD-PARTY-NOTICES
Lists every crate in the lock file (including ones only some platforms build); a crate with no license file is named, not skipped."""
import json
import pathlib
import sys

meta = json.load(sys.stdin)
workspace = set(meta["workspace_members"])
count = 0
for p in sorted(meta["packages"], key=lambda p: (p["name"], p["version"])):
    if p["id"] in workspace:
        continue
    count += 1
    print(f"{'=' * 70}\n{p['name']} {p['version']}  license: {p.get('license') or p.get('license_file') or 'UNKNOWN'}")
    if p.get("repository"):
        print(p["repository"])
    folder = pathlib.Path(p["manifest_path"]).parent
    files = sorted(f for f in folder.iterdir() if f.is_file() and f.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE")))
    for f in files:
        print(f"\n--- {f.name} ---\n{f.read_text(errors='replace').rstrip()}")
    if not files:
        print("\n(no license file in the crate; the license above is from its manifest)")
print(f"\n{count} crates", file=sys.stderr)
