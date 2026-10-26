// node --test scripts/release-notes.test.mjs  (npm run notes:test)
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { changelogEntry, releaseNotes } from "./release-notes.mjs";

const changelog = [
  "# Changelog",
  "",
  "intro",
  "",
  "## V1.2.4 (2026-01-02)",
  "",
  "Newer.",
  "",
  "### Added",
  "- **A:** b",
  "",
  "## V1.2.3 (2026-01-01)",
  "",
  "Older.",
  "",
].join("\r\n");

test("the entry is the lines under its heading, up to the next release", () => {
  const entry = changelogEntry(changelog, "1.2.4");
  assert.ok(entry.startsWith("Newer."));
  assert.ok(entry.includes("- **A:** b"));
  assert.ok(!entry.includes("Older."));
  assert.ok(!entry.includes("\r"));
});

test("a version with no entry is refused, not guessed", () => {
  assert.throws(() => changelogEntry(changelog, "1.2.5"), /no entry for V1\.2\.5/);
  assert.throws(() => changelogEntry("## V1.2.4 (x)\n\n## V1.2.3 (y)\nx", "1.2.4"), /empty/);
});

test("the page lists the four images, the three archives, the sums and the installers", () => {
  const page = releaseNotes({ changelog, version: "1.2.4", owner: "Acme", repo: "Acme/repomemo" });
  for (const name of ["server", "converter", "edge", "web"]) assert.ok(page.includes(`\`ghcr.io/acme/repomemo-${name}:1.2.4\``), name);
  for (const file of ["linux-x64.tar.gz", "linux-arm64.tar.gz", "windows-x64.zip"]) assert.ok(page.includes(`repomemo-server-1.2.4-${file}`), file);
  assert.ok(page.includes("SHA256SUMS"));
  assert.ok(page.includes("releases/download/v1.2.4/quickstart.sh | sh -s -- --version 1.2.4"));
  assert.ok(page.includes("repomemo-compose-1.2.4.tar.gz"));
  assert.ok(page.includes("service/install.sh") && page.includes("service\\install.ps1"));
  assert.ok(page.includes("`1.2` and `latest`"));
  assert.ok(page.includes("https://github.com/Acme/repomemo/blob/v1.2.4/docs/VERIFYING_IMAGES.md"));
});

test("the real changelog has an entry for the version in package.json, and the page is not empty", () => {
  const version = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version;
  const real = readFileSync(new URL("../CHANGELOG.md", import.meta.url), "utf8");
  const page = releaseNotes({ changelog: real, version, owner: "repomemo", repo: "repomemo/repomemo" });
  assert.ok(page.length > 500);
  assert.ok(!page.includes("TODO"));
});
