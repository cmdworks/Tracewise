// Run with: pnpm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { SEARCH_LIMIT, matchesQuery, mergeSearch, searchNote } from "./search.ts";

const row = (id: string, start: string, over: Record<string, unknown> = {}) =>
  ({ id, time: { start, end: start }, path: "a.txt", op: "modify", ...over }) as never;

test("a loaded event matches on path, reason, command or id, in any case", () => {
  assert.ok(matchesQuery(row("01ABC", "t1", { path: "src/Main.rs" }), "main"));
  assert.ok(matchesQuery(row("01ABC", "t1", { why: "Tighten the retry budget" }), "RETRY"));
  assert.ok(matchesQuery(row("01ABC", "t1", { command: "cargo test" }), "cargo"));
  assert.ok(matchesQuery(row("01ABCXYZ", "t1"), "abcx"));
  assert.ok(!matchesQuery(row("01ABC", "t1"), "nothing like this"));
  assert.ok(matchesQuery(row("01ABC", "t1"), "  "), "an empty query keeps everything");
});

test("Tracewise's matches and the loaded matches are one list, once each, newest first", () => {
  const fromTrail = [row("02", "2026-01-02", { why: "old reason match" }), row("01", "2026-01-01")];
  const loaded = [row("03", "2026-01-03", { path: "keep.txt" }), row("02", "2026-01-02", { why: "old reason match" }), row("04", "2026-01-04", { path: "other.rs" })];
  const out = mergeSearch(fromTrail, loaded, "keep");
  assert.deepEqual(out.map((c) => c.id), ["03", "02", "01"], "04 does not match, 02 is not repeated, newest first");
});

test("with nothing from Tracewise yet, the loaded matches are shown", () => {
  assert.deepEqual(mergeSearch([], [row("1", "t", { path: "x.rs" }), row("2", "t", { path: "y.rs" })], "x.rs").map((c) => c.id), ["1"]);
});

test("the note says how far the search reached", () => {
  const base = { query: "retry", loading: false, answeredFor: "retry", fromTrail: 3, shown: 3, failed: false };
  assert.equal(searchNote(base), "3 MATCHES IN THE WHOLE HISTORY");
  assert.equal(searchNote({ ...base, shown: 1, fromTrail: 1 }), "1 MATCH IN THE WHOLE HISTORY");
  assert.match(searchNote({ ...base, fromTrail: SEARCH_LIMIT, shown: SEARCH_LIMIT }), /NEWEST 200/);
  assert.match(searchNote({ ...base, loading: true }), /SEARCHING/);
  assert.match(searchNote({ ...base, answeredFor: "old" }), /SEARCHING/, "an answer to an earlier query is not this one's");
  assert.match(searchNote({ ...base, failed: true }), /LOADED EVENTS ONLY/);
  assert.equal(searchNote({ ...base, query: "" }), "");
});
