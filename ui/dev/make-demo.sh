#!/bin/bash
# A disposable repository with changes by a person and three agents, for trying the UI. Prints the two paths to use:
#   eval "$(dev/make-demo.sh /tmp/trail-ui-demo)"   then   TRAIL_UI_BRIDGE=1 TRAIL_BIN=$TRAIL_BIN TRAIL_HOME=$TRAIL_HOME pnpm dev
set -euo pipefail
root=${1:?usage: make-demo.sh <folder>}
bin=${TRAIL_BIN:-$(cd "$(dirname "$0")/../.." && pwd)/target/release/trail}
rm -rf "$root"; mkdir -p "$root/repo"; cd "$root/repo"
export TRAIL_HOME="$root/home"
git init -q
t() { env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID TRAIL_ACTOR="$1" "$bin" "${@:2}" >/dev/null; }
printf 'fn main() {}\n' > main.rs; printf '# Demo\n' > README.md
t human:me init
for i in 1 2 3 4 5 6; do
  printf 'fn main() {}\n// edit %s\n' "$i" > main.rs; t agent:claude:s1 scan
  printf '# Demo\nline %s\n' "$i" > README.md;       t agent:codex:s2 scan
  printf 'notes %s\n' "$i" > "notes$i.txt";          t human:me scan
done
printf 'binary\0data' > blob.bin;                   t agent:cursor:s3 scan
echo "export TRAIL_BIN=$bin"; echo "export TRAIL_HOME=$TRAIL_HOME"; echo "export DEMO_REPO=$root/repo"
