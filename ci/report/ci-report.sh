#!/bin/sh
# Tracewise in CI: fetch the shared history, verify it, and write the report of recent changes as Markdown.
# Fails (exit 1) when `tracewise verify` finds a changed or removed record. Usage: ci-report.sh [--since 7d]
# Needs `tracewise` on PATH. The history only exists in CI when sharing is on (`.tracewiseconfig`, `tracewise share init`).
set -eu
since=""
[ "${1:-}" = "--since" ] && since="${2:?--since needs a duration}"
out="${GITHUB_STEP_SUMMARY:-/dev/stdout}"
if [ -f .tracewiseconfig ]; then tracewise fetch >/dev/null || echo "tracewise: fetch failed; reporting what this checkout has" >&2; fi
if ! tracewise verify --json >/tmp/tracewise-verify.json 2>&1; then
  echo "Tracewise history failed verification:" >&2
  cat /tmp/tracewise-verify.json >&2
  exit 1
fi
{
  if [ -n "$since" ]; then tracewise report --since "$since"; else tracewise report; fi
} >>"$out"
