// Run with: node --test src/lib/result.test.ts
import assert from "node:assert/strict";
import { test } from "node:test";
import { errorText, failureText, parseJson } from "./result.ts";

const ok = (stdout: string) => ({ stdout, stderr: "", exit_code: 0 });

test("a failed call is an error with Tracewise's own words, not an empty list", () => {
  const res = { stdout: "", stderr: "trail: this repository has no Tracewise history yet; run `trail init` first\n", exit_code: 1 };
  assert.throws(() => parseJson("events", res), /no Tracewise history yet/);
  assert.equal(failureText("events", res).startsWith("trail:"), false, "the prefix is dropped");
});

test("a failure without a message names the call and the exit code", () => {
  assert.equal(failureText("events", { stdout: "", stderr: "", exit_code: 7 }), "events failed (exit 7)");
});

test("an empty answer and unreadable text are errors too", () => {
  assert.throws(() => parseJson("events", ok("  \n")), /printed nothing/);
  assert.throws(() => parseJson("events", ok("not json")), /could not be read/);
});

test("good JSON, including an empty array, is returned", () => {
  assert.deepEqual(parseJson("events", ok("[]")), []);
  assert.deepEqual(parseJson("status", ok('{"changes":3}')), { changes: 3 });
});

test("errorText accepts anything thrown", () => {
  assert.equal(errorText(new Error("x")), "x");
  assert.equal(errorText("plain"), "plain");
});
