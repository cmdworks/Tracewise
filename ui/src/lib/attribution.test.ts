// Run with: pnpm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { kindsText, shares, unnamedPercent } from "./attribution.ts";

const a = (confidence: Record<string, number>, actor_kind?: Record<string, number>) => ({
  days: 7,
  changes: Object.values(confidence).reduce((x, y) => x + y, 0),
  confidence,
  actor_kind
});

test("shares come in a fixed order, skip empty ones, and always add up to 100", () => {
  const s = shares(a({ unknown: 1, exact: 1, likely: 1 }));
  assert.deepEqual(s.map((x) => x.key), ["exact", "likely", "unknown"], "best first, whatever the input order");
  assert.equal(s.reduce((n, x) => n + x.percent, 0), 100, "33.3 x 3 is shown as 34 + 33 + 33, never 99");
  assert.deepEqual(s.map((x) => x.percent), [34, 33, 33]);
});

test("a confidence the view does not know is still shown, with an honest meaning", () => {
  const s = shares(a({ exact: 3, strange: 1 }));
  assert.deepEqual(s.map((x) => [x.key, x.percent]), [["exact", 75], ["strange", 25]]);
  assert.match(s[1].meaning, /does not describe/);
});

test("nothing to show for no summary or no changes", () => {
  assert.deepEqual(shares(null), []);
  assert.deepEqual(shares({ days: 7, changes: 0, confidence: {} }), []);
});

test("the unnamed share counts unknown and ambiguous", () => {
  assert.equal(unnamedPercent(a({ exact: 6, unknown: 3, ambiguous: 1 })), 40);
  assert.equal(unnamedPercent(a({ exact: 10 })), 0);
  assert.equal(unnamedPercent(null), 0);
});

test("actor kinds read as words, biggest first", () => {
  assert.equal(kindsText(a({ exact: 12 }, { human: 2, agent: 10 })), "10 by agents, 2 by you");
  assert.equal(kindsText(a({ exact: 1 })), "");
});
