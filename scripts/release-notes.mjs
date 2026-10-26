#!/usr/bin/env node
// The text of a release page (V0.4.39), written from the changelog so that nobody edits it by hand.
//
//   node scripts/release-notes.mjs [--version 0.4.39] [--owner repomemo] [--repo repomemo/repomemo]
//
// It prints the changelog entry of the version (without its heading), then what the release holds: the four images on
// GHCR, the three server archives with their checksum file and the installers inside them. The version defaults to the
// one in package.json. It reads files only.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

export const IMAGES = ["server", "converter", "edge", "web"];
export const ARCHIVES = [
  ["Linux x64", "linux", "x64", "tar.gz"],
  ["Linux arm64", "linux", "arm64", "tar.gz"],
  ["Windows x64", "windows", "x64", "zip"],
];

/** The changelog's entry for a version: the lines under `## V<version> (date)` up to the next `## `. */
export function changelogEntry(changelog, version) {
  const lines = changelog.replace(/\r\n/g, "\n").split("\n");
  const start = lines.findIndex((line) => new RegExp(`^## V${version.replace(/\./g, "\\.")} \\(`).test(line));
  if (start < 0) throw new Error(`CHANGELOG.md has no entry for V${version}`);
  let end = lines.findIndex((line, index) => index > start && line.startsWith("## "));
  if (end < 0) end = lines.length;
  const entry = lines.slice(start + 1, end).join("\n").trim();
  if (!entry) throw new Error(`the changelog entry of V${version} is empty`);
  return entry;
}

/** The whole page: the entry, then what the release holds. */
export function releaseNotes({ changelog, version, owner, repo }) {
  const registry = `ghcr.io/${owner.toLowerCase()}`;
  const lines = [changelogEntry(changelog, version), "", "---", "", "## What this release holds", ""];
  lines.push("### Container images", "");
  lines.push(`Four images for \`linux/amd64\` and \`linux/arm64\`, named \`${version}\`, \`${version.replace(/\.\d+$/, "")}\` and \`latest\`:`, "");
  for (const name of IMAGES) lines.push(`- \`${registry}/repomemo-${name}:${version}\``);
  lines.push("", `Each carries a signed build attestation and an SBOM: see [verifying an image](https://github.com/${repo}/blob/v${version}/docs/VERIFYING_IMAGES.md).`, "");
  lines.push("### Server archives", "");
  for (const [label, os, arch, extension] of ARCHIVES) lines.push(`- ${label}: \`repomemo-server-${version}-${os}-${arch}.${extension}\``);
  lines.push("- `SHA256SUMS`: check a download with `sha256sum --check --ignore-missing SHA256SUMS`", "");
  lines.push("### Installers", "");
  lines.push("- Linux: `sudo sh service/install.sh --generate-secrets --start` from the unpacked archive (a systemd service)");
  lines.push("- Windows: `.\\service\\install.ps1 -GenerateSecrets -Start` from the unpacked archive, in an administrator PowerShell (a Windows service)");
  lines.push(`- Upgrading and rolling back: [docs/DIRECT_UPGRADE.md](https://github.com/${repo}/blob/v${version}/docs/DIRECT_UPGRADE.md)`, "");
  return `${lines.join("\n").trimEnd()}\n`;
}

function option(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at > 0 && process.argv[at + 1] ? process.argv[at + 1] : fallback;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const version = option("version", JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version);
    const repo = option("repo", "repomemo/repomemo");
    const owner = option("owner", repo.split("/")[0]);
    process.stdout.write(releaseNotes({ changelog: readFileSync(join(root, "CHANGELOG.md"), "utf8"), version, owner, repo }));
  } catch (error) {
    console.error(`release-notes: ${error.message}`);
    process.exit(1);
  }
}
