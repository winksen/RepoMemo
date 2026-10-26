// node --test scripts/update-feed.test.mjs  (npm run feed:test)
import assert from "node:assert/strict";
import { test } from "node:test";
import { buildFeed } from "./update-feed.mjs";

const file = { platform: "windows-x86_64", url: "https://github.com/repomemo/repomemo/releases/download/v1.2.3/RepoMemo_1.2.3_x64-setup.exe", signature: " dW50cnVzdGVk\n" };

test("the feed names the version, the notes and, per platform, the file and its signature", () => {
  const feed = buildFeed({ version: "1.2.3", notes: "What changed.", date: "2026-10-26T00:00:00.000Z", files: [file] });
  assert.deepEqual(feed, { version: "1.2.3", notes: "What changed.", pub_date: "2026-10-26T00:00:00.000Z", platforms: { "windows-x86_64": { signature: "dW50cnVzdGVk", url: file.url } } });
});

test("a file with no signature, a plain-http address or an unknown platform is refused", () => {
  assert.throws(() => buildFeed({ version: "1.2.3", files: [{ ...file, signature: "  " }] }), /signature of windows-x86_64 is empty/);
  assert.throws(() => buildFeed({ version: "1.2.3", files: [{ ...file, url: file.url.replace("https", "http") }] }), /must be https/);
  assert.throws(() => buildFeed({ version: "1.2.3", files: [{ ...file, url: "https://u:p@example.com/x" }] }), /without a login/);
  assert.throws(() => buildFeed({ version: "1.2.3", files: [{ ...file, url: "not a url" }] }), /not a web address/);
  assert.throws(() => buildFeed({ version: "1.2.3", files: [{ ...file, platform: "plan9-x86" }] }), /not one of/);
  assert.throws(() => buildFeed({ version: "1.2.3", files: [file, file] }), /given twice/);
});

test("the version is a release number and there is at least one file", () => {
  assert.throws(() => buildFeed({ version: "latest", files: [file] }), /digits and dots/);
  assert.throws(() => buildFeed({ version: "1.2.3", files: [] }), /at least one file/);
});
