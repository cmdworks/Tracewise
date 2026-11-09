import { test } from "node:test";
import assert from "node:assert/strict";
import { buildTimelineActorOptions, matchesActorFilter, extractDistinctFiles, isBaselineChange } from "./actors.ts";
import type { ChangeItem } from "./types";

function mockChange(overrides: Partial<ChangeItem> = {}): ChangeItem {
  return {
    id: "01000000000000000000000000",
    time: { start: "2026-11-04T10:00:00Z", end: "2026-11-04T10:00:00Z" },
    path: "src/main.rs",
    op: "modify",
    actor: { kind: "agent", name: "claude" },
    candidates: [],
    confidence: "exact",
    source: "hook",
    ...overrides,
  };
}

test("buildTimelineActorOptions includes unknown history agents such as cursor and antigravity", () => {
  const changes: ChangeItem[] = [
    mockChange({ actor: { kind: "agent", name: "claude" } }),
    mockChange({ actor: { kind: "agent", name: "cursor" } }),
    mockChange({ actor: { kind: "agent", name: "antigravity" } }),
  ];

  const opts = buildTimelineActorOptions(changes);
  const ids = opts.map((o) => o.id);

  assert.ok(ids.includes("all"));
  assert.ok(ids.includes("you"));
  assert.ok(ids.includes("claude"));
  assert.ok(ids.includes("codex"));
  assert.ok(ids.includes("gemini"));
  assert.ok(ids.includes("cursor"));
  assert.ok(ids.includes("antigravity"));
});

test("matchesActorFilter filters human and extra agents correctly", () => {
  const humanChange = mockChange({ actor: { kind: "human", name: "me" } });
  const claudeChange = mockChange({ actor: { kind: "agent", name: "claude" } });
  const cursorChange = mockChange({ actor: { kind: "agent", name: "cursor" } });

  assert.equal(matchesActorFilter(humanChange, "all"), true);
  assert.equal(matchesActorFilter(humanChange, "you"), true);
  assert.equal(matchesActorFilter(humanChange, "claude"), false);

  assert.equal(matchesActorFilter(claudeChange, "claude"), true);
  assert.equal(matchesActorFilter(claudeChange, "cursor"), false);

  assert.equal(matchesActorFilter(cursorChange, "cursor"), true);
  assert.equal(matchesActorFilter(cursorChange, "codex"), false);
});

test("isBaselineChange identifies . and baseline op", () => {
  assert.equal(isBaselineChange({ path: ".", op: "baseline" }), true);
  assert.equal(isBaselineChange({ path: ".", op: "create" }), true);
  assert.equal(isBaselineChange({ path: "README.md", op: "baseline" }), true);
  assert.equal(isBaselineChange({ path: "README.md", op: "modify" }), false);
});

test("extractDistinctFiles skips baseline . row", () => {
  const changes: ChangeItem[] = [
    mockChange({ path: ".", op: "baseline" }),
    mockChange({ path: "src/main.rs", op: "modify" }),
    mockChange({ path: "src/main.rs", op: "create" }),
    mockChange({ path: "blob.bin", op: "create" }),
  ];

  const files = extractDistinctFiles(changes, (a) => a?.name || "unknown");
  const paths = files.map((f) => f.path);

  assert.deepEqual(paths, ["src/main.rs", "blob.bin"]);
  assert.equal(files.find((f) => f.path === "src/main.rs")?.count, 2);
});
