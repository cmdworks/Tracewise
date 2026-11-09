#!/bin/sh
# Tracewise in CI: fetch the shared history, verify it, and write the report of recent changes as Markdown.
# Fails (exit 1) when `trail verify` finds a changed or removed record. Usage: ci-report.sh [--since 7d]
# Needs `trail` on PATH. The history only exists in CI when sharing is on (`.trailconfig`, `trail share init`).
set -eu
since=""
[ "${1:-}" = "--since" ] && since="${2:?--since needs a duration}"
out="${GITHUB_STEP_SUMMARY:-/dev/stdout}"
if [ -f .trailconfig ]; then trail fetch >/dev/null || echo "trail: fetch failed; reporting what this checkout has" >&2; fi
if ! trail verify --json >/tmp/trail-verify.json 2>&1; then
  echo "Trail history failed verification:" >&2
  cat /tmp/trail-verify.json >&2
  exit 1
fi
{
  if [ -n "$since" ]; then trail report --since "$since"; else trail report; fi
} >>"$out"
