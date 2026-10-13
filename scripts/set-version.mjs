#!/usr/bin/env node
// The one place RepoMemo's version is set (roadmap C9).
//
//   node scripts/set-version.mjs 0.1.66   sets every file below to 0.1.66
//   node scripts/set-version.mjs --check  fails if any of them disagree (CI)
//
// Files: Cargo.toml ([workspace.package]), Cargo.lock (the workspace's own
// crates), package.json, package-lock.json (root and apps/desktop),
// apps/desktop/package.json and apps/desktop/src-tauri/tauri.conf.json.
// Only the version strings are rewritten, so formatting and line endings stay.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const VERSION = /^\d+\.\d+\.\d+$/;

/** Each target reads the versions it holds and can replace them. */
const targets = [
  {
    file: "Cargo.toml",
    pattern: /(\[workspace\.package\][\s\S]*?\nversion = ")([^"]+)(")/,
  },
  {
    file: "Cargo.lock",
    // A workspace crate: named repomemo-*, with no `source` (not from a registry).
    pattern: /(\[\[package\]\]\r?\nname = "repomemo-[^"]+"\r?\nversion = ")([^"]+)("\r?\n(?!source = ))/g,
  },
  { file: "package.json", pattern: /(^\{\s*"name": "repomemo",\s*"version": ")([^"]+)(")/ },
  { file: "apps/desktop/package.json", pattern: /(^\{\s*"name": "@repomemo\/desktop",\s*"version": ")([^"]+)(")/ },
  { file: "apps/desktop/src-tauri/tauri.conf.json", pattern: /("productName": "RepoMemo",\s*"version": ")([^"]+)(")/ },
  {
    file: "package-lock.json",
    pattern: /("name": "(?:repomemo|@repomemo\/desktop)",\s*"version": ")([^"]+)(")/g,
  },
];

function versionsIn(text, pattern) {
  const flags = pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`;
  return [...text.matchAll(new RegExp(pattern.source, flags))].map((match) => match[2]);
}

function check() {
  const found = new Map();
  for (const { file, pattern } of targets) {
    const versions = versionsIn(readFileSync(join(root, file), "utf8"), pattern);
    if (versions.length === 0) {
      console.error(`${file}: no version found; update scripts/set-version.mjs`);
      process.exit(1);
    }
    for (const version of versions) {
      found.set(version, [...(found.get(version) ?? []), file]);
    }
  }
  if (found.size !== 1) {
    console.error("The version is not the same everywhere:");
    for (const [version, files] of found) console.error(`  ${version}: ${[...new Set(files)].join(", ")}`);
    console.error("Run: node scripts/set-version.mjs <version>");
    process.exit(1);
  }
  console.log(`Version ${[...found.keys()][0]} everywhere.`);
}

function set(version) {
  if (!VERSION.test(version)) {
    console.error(`"${version}" is not a version like 0.1.66.`);
    process.exit(1);
  }
  for (const { file, pattern } of targets) {
    const path = join(root, file);
    const text = readFileSync(path, "utf8");
    if (versionsIn(text, pattern).length === 0) {
      console.error(`${file}: no version found; nothing was written for it`);
      process.exit(1);
    }
    writeFileSync(path, text.replace(pattern, (_, before, _old, after) => `${before}${version}${after}`));
    console.log(`${file} → ${version}`);
  }
}

const argument = process.argv[2];
if (argument === "--check") check();
else if (argument) set(argument);
else {
  console.error("Usage: node scripts/set-version.mjs <version> | --check");
  process.exit(1);
}
