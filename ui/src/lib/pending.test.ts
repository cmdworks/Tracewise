// Run with: pnpm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { pendingFrom } from "./pending.ts";

const row = (id: string, over: Record<string, unknown> = {}) =>
  ({ id, path: `${id}.txt`, op: "modify", source: "scan", intent: null, actor: { kind: "agent", name: "claude" }, time: { start: id, end: id }, ...over }) as never;

test("a change is pending when it has no intent and is not a baseline row, newest first as given", () => {
  const out = pendingFrom([row("c", { intent: "i1" }), row("b"), row("a", { source: "baseline" }), row("z")]);
  assert.deepEqual(out.items.map((p) => p.id), ["b", "z"]);
  assert.equal(out.more, false);
});

test("only the newest few are kept, and the page is told there are more", () => {
  const rows = Array.from({ length: 50 }, (_, i) => row(`id${String(i).padStart(2, "0")}`));
  const out = pendingFrom(rows, 30);
  assert.equal(out.items.length, 30);
  assert.equal(out.items[0].id, "id00");
  assert.equal(out.more, true);
});

test("exactly the limit is not 'more'", () => {
  assert.equal(pendingFrom(Array.from({ length: 30 }, (_, i) => row(`x${i}`)), 30).more, false);
});

test("an item carries what the Now tab shows: id, path, actor, session, time", () => {
  const [p] = pendingFrom([row("q")]).items;
  assert.deepEqual(Object.keys(p).sort(), ["actor", "id", "path", "session", "time"]);
});

test("the session comes from the actor, so the Now tab can group one task's changes", () => {
  const [a, b] = pendingFrom([row("a", { actor: { kind: "agent", name: "claude", session: "s-17" } }), row("b")]).items;
  assert.equal(a.session, "s-17");
  assert.equal(b.session, null);
});
