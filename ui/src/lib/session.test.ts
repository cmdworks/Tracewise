// Run with: pnpm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { parseSession, serializeSession } from "./session.ts";

test("what was saved comes back", () => {
  const s = { repo: "/Users/me/proj", tab: "files" as const, file: "src/a.rs" };
  assert.deepEqual(parseSession(serializeSession(s)), s);
});

test("nothing, damaged or wrong-shaped saves mean nothing saved, not an error", () => {
  const none = { repo: "", tab: "timeline", file: "" };
  assert.deepEqual(parseSession(null), none);
  assert.deepEqual(parseSession(""), none);
  assert.deepEqual(parseSession("{not json"), none);
  assert.deepEqual(parseSession('{"repo": 5, "tab": 7, "file": []}'), none);
});

test("an unknown tab falls back to the timeline but keeps the project", () => {
  assert.deepEqual(parseSession('{"repo":"/p","tab":"settings","file":""}'), { repo: "/p", tab: "timeline", file: "" });
});

test("a file without a project is dropped", () => {
  assert.equal(parseSession('{"repo":"","tab":"files","file":"a.rs"}').file, "");
});

test("with no project there is nothing to save, so an earlier save is not wiped", () => {
  assert.equal(serializeSession({ repo: "", tab: "files", file: "a.rs" }), null);
  assert.equal(serializeSession({ repo: "   ", tab: "timeline", file: "" }), null);
});
