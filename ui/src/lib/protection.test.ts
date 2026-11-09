import test from "node:test";
import assert from "node:assert/strict";
import { alerts, level, noticesFrom } from "./protection.ts";

const base = { ok: true, profile: "core", problems: [] as string[] };

test("the level follows what is running", () => {
  assert.equal(level(null).rank, 0);
  assert.equal(level({ ...base, coverage: { daemon: false } }).rank, 0);
  assert.equal(level({ ...base, coverage: { daemon: true } }).rank, 1);
  const attributed = { daemon: true, shell: true, git_wrapper: true, agents: { claude: true, codex: false } };
  assert.equal(level({ ...base, coverage: attributed }).rank, 2);
  assert.equal(level({ ...base, coverage: { ...attributed, strict: { active: true } } }).rank, 3);
  assert.equal(level({ ...base, coverage: { ...attributed, agents: { claude: false } } }).rank, 1);
});

test("alerts name the holes and carry Trail's own notices", () => {
  const d = {
    ...base,
    problems: ["db is slow"],
    coverage: { daemon: true, agents: { claude: true, codex: false }, missing_events: { claude: ["post-edit"] }, strict: { installed: true, running: false } }
  };
  const notes = noticesFrom([
    { id: "1", kind: "git", text: "git status", start: "2026-11-07T01:00:00Z" },
    { id: "2", kind: "unhooked", text: "2 changes possibly by claude", start: "2026-11-07T02:00:00Z" },
    { id: "3", kind: "gap", text: "Trail was not running", start: "2026-11-07T03:00:00Z" }
  ]);
  assert.deepEqual(notes.map((n) => n.id), ["3", "2"]);
  const a = alerts(d, notes);
  assert.equal(a.length, 6);
  assert.match(a[0], /No hooks for codex/);
  assert.match(a[a.length - 1], /possibly by claude/);
  assert.deepEqual(alerts(null), []);
});
