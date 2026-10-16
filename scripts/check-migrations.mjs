#!/usr/bin/env node
// Keeps a released database migration from changing (roadmap R9).
//
//   node scripts/check-migrations.mjs          (npm run migrations:check)
//   node scripts/check-migrations.mjs --lock   adds the migrations that are not in the lock yet
//
// A migration that has run in someone's environment is recorded there with a checksum, and the server
// refuses to open an environment whose recorded checksum differs from the file it carries. So a
// released migration must never be edited: a change goes into a new migration. This reads
// `crates/storage/migrations.lock` (the SHA-256 of each released file, line ends as LF) and fails when
//
//   - a file in the lock is missing or no longer matches its line;
//   - the files are not numbered 0001, 0002, ... without a gap or a repeat;
//   - a migration is not in the lock yet (it ships with this change: `--lock` adds it).
//
// The upgrade test in the storage crate (`an_environment_made_by_v0_2_0_upgrades_in_place_and_keeps_its_data`)
// then proves that an environment of 0.2.0 opens and keeps its data.

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const directory = join(root, "crates", "storage", "migrations");
const lockPath = join(root, "crates", "storage", "migrations.lock");

const sha256 = (path) => createHash("sha256").update(readFileSync(path, "utf8").replace(/\r\n/g, "\n")).digest("hex");

const files = readdirSync(directory).filter((name) => name.endsWith(".sql")).sort();
const lockText = readFileSync(lockPath, "utf8");
const locked = new Map();
for (const line of lockText.split(/\r?\n/)) {
  if (!line.trim() || line.startsWith("#")) continue;
  const [release, name, hash] = line.trim().split(/\s+/);
  locked.set(name, { release, hash });
}

const problems = [];

files.forEach((name, index) => {
  const expected = String(index + 1).padStart(4, "0");
  if (!name.startsWith(`${expected}_`)) problems.push(`${name} should be numbered ${expected}: migrations count 0001, 0002, ... with no gap and no repeat`);
});

for (const [name, entry] of locked) {
  if (!files.includes(name)) {
    problems.push(`${name} (released in ${entry.release}) is gone; a released migration is never removed or renamed`);
  } else if (sha256(join(directory, name)) !== entry.hash) {
    problems.push(`${name} (released in ${entry.release}) was changed; a released migration is never edited: put the change in a new migration`);
  }
}

const unlocked = files.filter((name) => !locked.has(name));

if (process.argv.includes("--lock")) {
  if (problems.length) {
    for (const problem of problems) console.error(`✗ ${problem}`);
    process.exit(1);
  }
  const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
  const added = unlocked.map((name) => `${version}  ${name}  ${sha256(join(directory, name))}`);
  if (added.length) writeFileSync(lockPath, `${lockText.replace(/\s*$/, "\n")}${added.join("\n")}\n`);
  console.log(added.length ? `Locked ${added.length} migration${added.length === 1 ? "" : "s"} for ${version}: ${unlocked.join(", ")}` : "Every migration is locked already.");
  process.exit(0);
}

for (const name of unlocked) problems.push(`${name} is not in crates/storage/migrations.lock: run node scripts/check-migrations.mjs --lock to record it with this change`);

if (problems.length) {
  for (const problem of problems) console.error(`✗ ${problem}`);
  process.exit(1);
}
console.log(`Migrations: ${files.length} files, every one locked and unchanged since its release.`);
