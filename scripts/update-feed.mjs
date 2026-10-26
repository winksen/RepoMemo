#!/usr/bin/env node
// Writes the update feed of a release (V0.4.56): the `latest.json` the app's updater reads.
//
//   node scripts/update-feed.mjs --version 0.4.56 --notes release-notes.md --out latest.json \
//     --platform windows-x86_64 --url https://github.com/.../RepoMemo_0.4.56_x64-setup.exe --signature RepoMemo_0.4.56_x64-setup.exe.sig
//
// `--platform --url --signature` can be repeated, one set per file. The signature is the text of the `.sig` file that `tauri build`
// writes when the signing key is given; a feed with an empty one, or an address that is not https, is refused.

import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const PLATFORMS = ["windows-x86_64", "windows-aarch64", "linux-x86_64", "linux-aarch64", "darwin-x86_64", "darwin-aarch64"];

/** The feed for these files; throws what is wrong. `files` is [{ platform, url, signature }]. */
export function buildFeed({ version, notes = "", date = new Date(0).toISOString(), files }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`the version (${version}) is digits and dots, such as 0.4.56`);
  if (!files || files.length === 0) throw new Error("give at least one file: --platform, --url and --signature");
  const platforms = {};
  for (const { platform, url, signature } of files) {
    if (!PLATFORMS.includes(platform)) throw new Error(`the platform ${platform} is not one of ${PLATFORMS.join(", ")}`);
    if (platforms[platform]) throw new Error(`the platform ${platform} is given twice`);
    let parsed;
    try {
      parsed = new URL(url);
    } catch {
      throw new Error(`the address of ${platform} (${url}) is not a web address`);
    }
    if (parsed.protocol !== "https:" || parsed.username || parsed.password) throw new Error(`the address of ${platform} must be https:// without a login`);
    const signed = String(signature ?? "").trim();
    if (!signed) throw new Error(`the signature of ${platform} is empty; build with the signing key`);
    platforms[platform] = { signature: signed, url: parsed.href };
  }
  return { version, notes, pub_date: date, platforms };
}

function all(name) {
  const found = [];
  process.argv.forEach((value, at) => {
    if (value === `--${name}` && process.argv[at + 1]) found.push(process.argv[at + 1]);
  });
  return found;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const [platforms, urls, signatures] = [all("platform"), all("url"), all("signature")];
    if (platforms.length !== urls.length || urls.length !== signatures.length) throw new Error("each file needs a --platform, a --url and a --signature");
    const notesPath = all("notes")[0];
    const feed = buildFeed({
      version: all("version")[0],
      notes: notesPath ? readFileSync(notesPath, "utf8").trim() : "",
      date: process.env.SOURCE_DATE_EPOCH ? new Date(Number(process.env.SOURCE_DATE_EPOCH) * 1000).toISOString() : new Date().toISOString(),
      files: platforms.map((platform, index) => ({ platform, url: urls[index], signature: readFileSync(signatures[index], "utf8") })),
    });
    writeFileSync(all("out")[0] ?? "latest.json", `${JSON.stringify(feed, null, 2)}\n`);
    console.log(`update feed: ${Object.keys(feed.platforms).join(", ")} for ${feed.version}`);
  } catch (error) {
    console.error(`update-feed: ${error.message}`);
    process.exit(1);
  }
}
