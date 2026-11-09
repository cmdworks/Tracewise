import { test } from "node:test";
import assert from "node:assert/strict";
import { linesFromChange, survivalNote } from "./blame-view.ts";

const l = (line: number, change: string | null) => ({ line, text: "", change });

test("finds the current lines that still come from a change", () => {
  const lines = [l(1, "a"), l(2, "b"), l(3, "b"), l(4, null)];
  assert.deepEqual(linesFromChange(lines, "b"), [2, 3]);
  assert.deepEqual(linesFromChange(lines, "z"), []);
  assert.deepEqual(linesFromChange(lines, null), []);
});

test("says how many lines survive, including none and all", () => {
  assert.equal(survivalNote(0, 11), "None of this change's lines are in the file now; later changes replaced or removed them.");
  assert.equal(survivalNote(3, 11), "3 of 11 lines in the file now come from this change.");
  assert.equal(survivalNote(11, 11), "All 11 lines come from this change.");
  assert.equal(survivalNote(1, 1), "The file's only line comes from this change.");
  assert.equal(survivalNote(0, 0), "");
});
