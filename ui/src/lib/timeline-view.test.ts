import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  shortenPathFront,
  formatEventRelativeTime,
  getDateGroupLabel,
  flattenTimelineWithHeaders,
} from "./timeline-view.ts";
import { headerAt, opLetter } from "./timeline-view.ts";
import type { ChangeItem } from "./types.ts";

describe("timeline-view helpers", () => {
  it("shortenPathFront truncates leading path segments keeping last 2", () => {
    assert.equal(
      shortenPathFront("crates/tracewise-cli/src/cmd_history/revert.rs"),
      "…/cmd_history/revert.rs"
    );
    assert.equal(shortenPathFront("ui/src/lib/theme.css"), "…/lib/theme.css");
    assert.equal(shortenPathFront("src/main.rs"), "src/main.rs");
    assert.equal(shortenPathFront("README.md"), "README.md");
    assert.equal(shortenPathFront("."), ".");
    assert.equal(shortenPathFront(""), "");
  });

  it("formatEventRelativeTime produces clean relative strings", () => {
    const now = new Date("2026-11-06T12:00:00Z");
    const sec30 = new Date("2026-11-06T11:59:30Z").toISOString();
    const min2 = new Date("2026-11-06T11:58:00Z").toISOString();
    const min14 = new Date("2026-11-06T11:46:00Z").toISOString();
    const hour1 = new Date("2026-11-06T11:00:00Z").toISOString();
    const hours3 = new Date("2026-11-06T09:00:00Z").toISOString();
    const day1 = new Date("2026-11-05T12:00:00Z").toISOString();

    assert.equal(formatEventRelativeTime(sec30, now), "just now");
    assert.equal(formatEventRelativeTime(min2, now), "2 min ago");
    assert.equal(formatEventRelativeTime(min14, now), "14 min ago");
    assert.equal(formatEventRelativeTime(hour1, now), "1 hour ago");
    assert.equal(formatEventRelativeTime(hours3, now), "3 hours ago");
    assert.equal(formatEventRelativeTime(day1, now), "1 day ago");
  });

  it("getDateGroupLabel labels today, yesterday and older dates", () => {
    const now = new Date("2026-11-06T12:00:00Z");
    const today = new Date("2026-11-06T08:00:00Z").toISOString();
    const yesterday = new Date("2026-11-05T15:00:00Z").toISOString();
    const older = new Date("2026-10-28T10:00:00Z").toISOString();

    assert.equal(getDateGroupLabel(today, now), "TODAY");
    assert.equal(getDateGroupLabel(yesterday, now), "YESTERDAY");
    assert.equal(getDateGroupLabel(older, now), "OCT 28, 2026");
  });

  it("flattenTimelineWithHeaders groups changes under date headers", () => {
    const now = new Date("2026-11-06T12:00:00Z");
    const sampleChanges: ChangeItem[] = [
      {
        id: "c1",
        op: "modify",
        path: "crates/tracewise-cli/src/main.rs",
        actor: { kind: "claude", name: "Claude" },
        time: { start: "2026-11-06T11:30:00Z", end: "2026-11-06T11:30:00Z" },
        why: "Fix bug",
      },
      {
        id: "c2",
        op: "create",
        path: "ui/src/lib/test.ts",
        actor: { kind: "human", name: "You" },
        time: { start: "2026-11-06T10:00:00Z", end: "2026-11-06T10:00:00Z" },
        why: "Add test",
      },
      {
        id: "c3",
        op: "delete",
        path: "old.txt",
        actor: { kind: "codex", name: "Codex" },
        time: { start: "2026-11-05T18:00:00Z", end: "2026-11-05T18:00:00Z" },
        why: "Cleanup",
      },
    ];

    const flattened = flattenTimelineWithHeaders(sampleChanges, now);
    assert.equal(flattened.length, 5); // Header TODAY + 2 changes + Header YESTERDAY + 1 change
    assert.equal(flattened[0].kind, "header");
    if (flattened[0].kind === "header") {
      assert.equal(flattened[0].label, "TODAY");
      assert.equal(flattened[0].count, 2);
    }
    assert.equal(flattened[1].kind, "change");
    assert.equal(flattened[2].kind, "change");
    assert.equal(flattened[3].kind, "header");
    if (flattened[3].kind === "header") {
      assert.equal(flattened[3].label, "YESTERDAY");
      assert.equal(flattened[3].count, 1);
    }
    assert.equal(flattened[4].kind, "change");
  });
});

describe("pinned day header and op letters", () => {
  const h = (label: string) => ({ kind: "header" as const, id: `header:${label}`, label, count: 1 });
  const c = (id: string) => ({ kind: "change" as const, id, change: { id } as never });
  const items = [h("TODAY"), c("a"), c("b"), h("YESTERDAY"), c("d")];

  it("finds the header that owns a row", () => {
    assert.equal(headerAt(items, 0)?.label, "TODAY");
    assert.equal(headerAt(items, 2)?.label, "TODAY");
    assert.equal(headerAt(items, 4)?.label, "YESTERDAY");
    assert.equal(headerAt(items, 99)?.label, "YESTERDAY");
    assert.equal(headerAt([c("x")], 0), null);
  });

  it("shortens operations to one letter", () => {
    assert.deepEqual(["modify", "create", "delete", "rename", "baseline", ""].map(opLetter), ["M", "C", "D", "R", "B", "?"]);
  });
});
