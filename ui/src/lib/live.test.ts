import { test } from "node:test";
import assert from "node:assert/strict";
import { decideReload, formatRelativeTime, isStatusStale } from "./live.ts";
import type { TrailStatus } from "./types";

function mockStatus(overrides: Partial<TrailStatus> = {}): TrailStatus {
  return {
    repo: "/path/to/repo",
    data_dir: "/path/to/repo/.git/trail",
    changes: 10,
    locks: 0,
    daemon: "running",
    profile: {
      name: "core",
      core: true,
      coordination: false,
      policy: false,
      share: false,
      strict: false,
    },
    repo_id: "01000000000000000000000000",
    ...overrides,
  };
}

test("decideReload reloads everything on initial status or repo change", () => {
  const next = mockStatus({ changes: 15 });
  const initialDecision = decideReload(null, next);
  assert.equal(initialDecision.reloadTimeline, true);
  assert.equal(initialDecision.reloadPending, true);
  assert.equal(initialDecision.reloadLocks, true);
  assert.equal(initialDecision.reloadAgents, true);

  const prev = mockStatus({ repo: "/path/one" });
  const nextRepo = mockStatus({ repo: "/path/two" });
  const switchDecision = decideReload(prev, nextRepo);
  assert.equal(switchDecision.reloadTimeline, true);
  assert.equal(switchDecision.reloadPending, true);
  assert.equal(switchDecision.reloadLocks, true);
  assert.equal(switchDecision.reloadAgents, true);
});

test("decideReload detects changes moved and computes new count", () => {
  const prev = mockStatus({ changes: 10, locks: 0 });
  const next = mockStatus({ changes: 13, locks: 0 });

  const decision = decideReload(prev, next);
  assert.equal(decision.reloadTimeline, true);
  assert.equal(decision.reloadPending, true);
  assert.equal(decision.reloadLocks, false);
  assert.equal(decision.newChangesCount, 3);
});

test("decideReload detects lock count moved", () => {
  const prev = mockStatus({ changes: 10, locks: 0 });
  const next = mockStatus({ changes: 10, locks: 2 });

  const decision = decideReload(prev, next);
  assert.equal(decision.reloadTimeline, false);
  assert.equal(decision.reloadPending, false);
  assert.equal(decision.reloadLocks, true);
  assert.equal(decision.newChangesCount, 0);
});

test("decideReload does nothing when snapshots are identical", () => {
  const prev = mockStatus({ changes: 10, locks: 1 });
  const next = mockStatus({ changes: 10, locks: 1 });

  const decision = decideReload(prev, next);
  assert.equal(decision.reloadTimeline, false);
  assert.equal(decision.reloadPending, false);
  assert.equal(decision.reloadLocks, false);
  assert.equal(decision.newChangesCount, 0);
});

test("formatRelativeTime formats seconds, minutes and hours correctly", () => {
  const base = new Date("2026-11-04T12:00:00Z");
  assert.equal(formatRelativeTime(base, new Date("2026-11-04T12:00:02Z")), "updated just now");
  assert.equal(formatRelativeTime(base, new Date("2026-11-04T12:00:15Z")), "updated 15s ago");
  assert.equal(formatRelativeTime(base, new Date("2026-11-04T12:03:00Z")), "updated 3m ago");
  assert.equal(formatRelativeTime(base, new Date("2026-11-04T14:00:00Z")), "updated 2h ago");
});

test("isStatusStale flags stale updates after threshold", () => {
  const base = new Date("2026-11-04T12:00:00Z");
  assert.equal(isStatusStale(null), true);
  assert.equal(isStatusStale(base, new Date("2026-11-04T12:00:05Z"), 10), false);
  assert.equal(isStatusStale(base, new Date("2026-11-04T12:00:12Z"), 10), true);
});
