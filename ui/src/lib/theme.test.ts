import test from "node:test";
import assert from "node:assert/strict";
import { next, resolve } from "./theme.ts";

test("system follows the operating system, the others do not", () => {
  assert.equal(resolve("system", true), "dark");
  assert.equal(resolve("system", false), "light");
  assert.equal(resolve("light", true), "light");
  assert.equal(resolve("dark", false), "dark");
});

test("the switch cycles system, light, dark", () => {
  assert.equal(next("system"), "light");
  assert.equal(next("light"), "dark");
  assert.equal(next("dark"), "system");
});
