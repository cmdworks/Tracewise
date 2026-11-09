import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { groupPendingChanges } from "./now-grouping.ts";
import type { PendingItem } from "./types.ts";

describe("now-grouping helpers", () => {
  it("groups items with the same session together", () => {
    const items: PendingItem[] = [
      {
        id: "c1",
        path: "src/a.rs",
        actor: { kind: "claude", name: "Claude" },
        session: "s_41b",
        time: { start: "2026-11-06T10:00:00Z", end: "2026-11-06T10:00:00Z" },
      },
      {
        id: "c2",
        path: "src/b.rs",
        actor: { kind: "claude", name: "Claude" },
        session: "s_41b",
        time: { start: "2026-11-06T10:01:00Z", end: "2026-11-06T10:01:00Z" },
      },
      {
        id: "c3",
        path: "src/c.rs",
        actor: { kind: "codex", name: "Codex" },
        session: "s_77e",
        time: { start: "2026-11-06T10:02:00Z", end: "2026-11-06T10:02:00Z" },
      },
    ];

    const groups = groupPendingChanges(items);
    assert.equal(groups.length, 2);
    assert.equal(groups[0].id, "session:s_41b");
    assert.equal(groups[0].items.length, 2);
    assert.equal(groups[1].id, "session:s_77e");
    assert.equal(groups[1].items.length, 1);
  });

  it("groups items without session by actor", () => {
    const items: PendingItem[] = [
      {
        id: "c1",
        path: "src/a.rs",
        actor: { kind: "human", name: "You" },
        time: { start: "2026-11-06T10:00:00Z", end: "2026-11-06T10:00:00Z" },
      },
      {
        id: "c2",
        path: "src/b.rs",
        actor: { kind: "human", name: "You" },
        time: { start: "2026-11-06T10:01:00Z", end: "2026-11-06T10:01:00Z" },
      },
    ];

    const groups = groupPendingChanges(items);
    assert.equal(groups.length, 1);
    assert.equal(groups[0].id, "actor:you");
    assert.equal(groups[0].items.length, 2);
  });
});
