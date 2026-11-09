// Run with: pnpm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { repoOrNull } from "./api.ts";

test("no project chosen yet is no repository, never an empty path (the desktop side rejects '')", () => {
  assert.equal(repoOrNull(""), null);
  assert.equal(repoOrNull("   "), null);
  assert.equal(repoOrNull(undefined), null);
  assert.equal(repoOrNull(null), null);
});

test("a chosen project is sent as it is", () => {
  assert.equal(repoOrNull("/Users/me/project"), "/Users/me/project");
});
