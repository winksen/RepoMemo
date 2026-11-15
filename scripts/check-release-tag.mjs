#!/usr/bin/env node
// Checks a release tag before it is pushed (V0.6.17).
//
//   node scripts/check-release-tag.mjs v0.6.17      (npm run tag:check -- v0.6.17)
//   node scripts/check-release-tag.mjs --existing   lists the tags in this clone and says which can start a release
//
// A release tag is `v` plus the version in package.json: `v0.6.17` for 0.6.17. That is the form every release workflow
// starts from (`on: push: tags: ["v*"]`) and checks (`test "$GITHUB_REF_NAME" = "v<package version>"`). The older tags
// `RM-V0.2.0`, `RM-V0.3.0` and `RM-V0.4.0` stay as history and never start a workflow. Nothing here creates or pushes a tag.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const RELEASE_TAG = /^v(\d+)\.(\d+)\.(\d+)$/;

/** What is wrong with `tag` for a release of `version`, or an empty list when it can start one. */
export function tagProblems(tag, version) {
  const problems = [];
  if (!RELEASE_TAG.test(tag)) {
    problems.push(`"${tag}" is not a release tag: use v<major>.<minor>.<patch>, for example v${version}. A tag that does not start with "v" never starts a release workflow.`);
  } else if (tag !== `v${version}`) {
    problems.push(`"${tag}" does not match the product version ${version}: the workflows refuse a tag that is not exactly v${version}.`);
  }
  return problems;
}

/** Tags in a list, split into the ones that can start a release and the ones kept as history. */
export function classifyTags(tags) {
  return {
    release: tags.filter((tag) => RELEASE_TAG.test(tag)),
    history: tags.filter((tag) => !RELEASE_TAG.test(tag)),
  };
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = () => JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const argument = process.argv[2];
  if (argument === "--existing") {
    const tags = execFileSync("git", ["tag", "--list"], { cwd: root, encoding: "utf8" }).split("\n").filter(Boolean);
    const { release, history } = classifyTags(tags);
    console.log(`Tags that can start a release: ${release.length ? release.join(", ") : "none"}`);
    console.log(`Tags kept as history (they start nothing): ${history.length ? history.join(", ") : "none"}`);
  } else if (!argument) {
    console.error("Usage: node scripts/check-release-tag.mjs <tag> | --existing");
    process.exit(2);
  } else {
    const problems = tagProblems(argument, version());
    if (problems.length) {
      for (const problem of problems) console.error(`✗ ${problem}`);
      process.exit(1);
    }
    console.log(`${argument} can start a release: it is v plus the product version ${version()}. Nothing was tagged or pushed.`);
  }
}
