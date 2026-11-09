import test from "node:test";
import assert from "node:assert/strict";
import { rowsFrom, setArgs, sourceText, SETTINGS, shareFrom, shareInitArgs, shareProfileOff } from "./settings.ts";

test("rows follow the order of the definitions and drop what Tracewise did not return", () => {
  const rows = rowsFrom([
    { key: "storage.secrets", value: "hash", source: "default" },
    { key: "reasons.mode", value: "enforce", source: "project" },
    { key: "unknown.key", value: "x", source: "default" },
    { nope: true }
  ]);
  assert.deepEqual(rows.map((r) => r.key), ["reasons.mode", "storage.secrets"]);
  assert.equal(rows[0].source, "project");
  assert.deepEqual(rowsFrom(null), []);
});

test("save arguments", () => {
  assert.deepEqual(setArgs("reasons.mode", " enforce ", false), ["config", "set", "reasons.mode", "enforce"]);
  assert.deepEqual(setArgs("retention.commands", "14d", true), ["config", "set", "--global", "retention.commands", "14d"]);
  assert.equal(sourceText("global"), "all projects");
  assert.equal(SETTINGS.length, 6);
});

test("share status is read defensively and never carries a key", () => {
  assert.deepEqual(shareFrom(null), { configured: false, remote: undefined, store: undefined, level: undefined, refs: undefined, encrypt: false, key_id: null, key_exported: false, trusted: false, warnings: [] });
  const s = shareFrom({ configured: true, remote: "git@h:o/r.git", store: "origin", level: "summary", refs: "hidden", encrypt: true, key_id: "abc", key_exported: false, trusted: true, warnings: ["x", 5] });
  assert.equal(s.store, "origin");
  assert.deepEqual(s.warnings, ["x"]);
  assert.equal(s.key_id, "abc");
});

test("share init arguments and the profile refusal", () => {
  assert.deepEqual(shareInitArgs({ remote: "", level: "summary", encrypt: false }), ["share", "init", "--level", "summary"]);
  assert.deepEqual(shareInitArgs({ remote: " git@h:o/r.git ", level: "full", encrypt: true }), ["share", "init", "--level", "full", "--remote", "git@h:o/r.git", "--encrypt"]);
  assert.ok(shareProfileOff("tracewise: share profile is not enabled; add '[profile] share = true'"));
  assert.ok(!shareProfileOff("tracewise: something else"));
});
