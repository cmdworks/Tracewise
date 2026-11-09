// Run with: pnpm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { recentUndos } from "./undos.ts";

const row = (id: string, op: string, path: string, start: string, over: Record<string, unknown> = {}) =>
  ({ id, op, path, time: { start, end: start }, ...over }) as never;

test("only undo records are listed, newest first, with the change they put back", () => {
  const changes = [
    row("u2", "undo", "b.txt", "2026-01-04", { undoes: ["c2"] }),
    row("c3", "modify", "a.txt", "2026-01-03"),
    row("u1", "undo", "a.txt", "2026-01-02", { undoes: ["c1"] }),
  ];
  const out = recentUndos(changes);
  assert.deepEqual(out.map((e) => [e.undo.id, e.undid]), [["u2", "c2"], ["u1", "c1"]]);
});

test("a later redo of the same file marks the undo as likely redone, an earlier one or another file does not", () => {
  const changes = [
    row("r1", "redo", "a.txt", "2026-01-05"),
    row("u2", "undo", "b.txt", "2026-01-04"),
    row("u1", "undo", "a.txt", "2026-01-03"),
    row("r0", "redo", "a.txt", "2026-01-01"),
  ];
  const byId = Object.fromEntries(recentUndos(changes).map((e) => [e.undo.id, e.likelyRedone]));
  assert.deepEqual(byId, { u2: false, u1: true });
});

test("an undo that does not name its change is still listed, and the list is capped", () => {
  const many = Array.from({ length: 30 }, (_, i) => row(`u${i}`, "undo", `f${i}`, `2026-02-${String(30 - i).padStart(2, "0")}`));
  const out = recentUndos(many, 20);
  assert.equal(out.length, 20);
  assert.equal(out[0].undid, null);
});
