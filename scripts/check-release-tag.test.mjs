import assert from "node:assert/strict";
import { test } from "node:test";
import { classifyTags, tagProblems } from "./check-release-tag.mjs";

test("a tag that is v plus the product version can start a release", () => {
  assert.deepEqual(tagProblems("v0.6.17", "0.6.17"), []);
});

test("the old RM-V form and a tag for another version are refused with the reason", () => {
  assert.match(tagProblems("RM-V0.4.0", "0.6.17")[0], /not a release tag/);
  assert.match(tagProblems("0.6.17", "0.6.17")[0], /not a release tag/);
  assert.match(tagProblems("v0.6.16", "0.6.17")[0], /does not match the product version/);
  assert.match(tagProblems("v0.6", "0.6.17")[0], /not a release tag/);
});

test("existing tags are split into the ones that start a release and history", () => {
  assert.deepEqual(classifyTags(["RM-V0.2.0", "v0.5.0", "RM-V0.4.0"]), { release: ["v0.5.0"], history: ["RM-V0.2.0", "RM-V0.4.0"] });
});
