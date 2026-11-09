// Run with: pnpm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { scrollTopToReveal, windowFor } from "./virtual.ts";

const w = (over: Partial<Parameters<typeof windowFor>[0]> = {}) => windowFor({ scrollTop: 0, height: 300, rowH: 30, count: 1000, overscan: 5, ...over });

test("at the top only the visible rows and the overscan below are rendered", () => {
  assert.deepEqual(w(), { start: 0, end: 15, padTop: 0, padBottom: 985 * 30 });
});

test("in the middle there is room above and below, and the pads plus the rows are the whole list", () => {
  const x = w({ scrollTop: 3000 });
  assert.deepEqual([x.start, x.end], [95, 115]);
  assert.equal(x.padTop + (x.end - x.start) * 30 + x.padBottom, 1000 * 30);
});

test("at the very bottom the window ends at the last row", () => {
  const x = w({ scrollTop: 1000 * 30 - 300 });
  assert.equal(x.end, 1000);
  assert.equal(x.padBottom, 0);
});

test("an empty list, a zero height row and a scroll past the end do not break", () => {
  assert.deepEqual(w({ count: 0 }), { start: 0, end: 0, padTop: 0, padBottom: 0 });
  assert.deepEqual(w({ rowH: 0 }), { start: 0, end: 0, padTop: 0, padBottom: 0 });
  const past = w({ scrollTop: 10_000_000 });
  assert.ok(past.start <= past.end && past.end <= 1000);
});

test("the rendered rows are a handful however long the list is", () => {
  const x = w({ count: 100_000, scrollTop: 1_500_000 });
  assert.ok(x.end - x.start <= 25, `${x.end - x.start} rows rendered`);
});

test("a row already in view needs no scrolling; one above or below is brought in", () => {
  const base = { scrollTop: 300, height: 300, rowH: 30 };
  assert.equal(scrollTopToReveal({ ...base, index: 15 }), null);
  assert.equal(scrollTopToReveal({ ...base, index: 3 }), 90, "above: its top becomes the top");
  assert.equal(scrollTopToReveal({ ...base, index: 25 }), 480, "below: its bottom becomes the bottom");
  assert.equal(scrollTopToReveal({ scrollTop: 100, height: 300, rowH: 30, index: 0 }), 0);
});
