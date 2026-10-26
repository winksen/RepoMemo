// node --test scripts/check-desktop.test.mjs  (npm run desktop:parity:test)
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { checkDesktop } from "./check-desktop.mjs";

const REASON = "Only the app has this to show";

function app(files, list = { files: {}, styles: {} }) {
  const root = mkdtempSync(join(tmpdir(), "repomemo-parity-"));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  writeFileSync(join(root, "desktop-only.json"), JSON.stringify(list));
  return root;
}

test("a web client with nothing for the desktop passes", () => {
  assert.deepEqual(checkDesktop(app({ "src/web/Page.tsx": "export const a = 1;", "src/styles/a.css": ".page { color: red; }" })), []);
});

test("a screen named for the desktop, or one that calls the shell, fails until it is listed with a reason", () => {
  const named = app({ "src/components/DesktopThing.tsx": "export const a = 1;" });
  assert.match(checkDesktop(named)[0], /DesktopThing.tsx exists for the desktop alone/);
  const calls = app({ "src/web/Page.tsx": 'import { invoke } from "@tauri-apps/api/core";' });
  assert.match(checkDesktop(calls)[0], /src\/web\/Page.tsx exists for the desktop alone/);
  const imports = app({ "src/web/Page.tsx": 'import { sessionStore } from "../lib/desktop";' });
  assert.equal(checkDesktop(imports).length, 1);
  const listed = app({ "src/components/DesktopThing.tsx": "export const a = 1;" }, { files: { "src/components/DesktopThing.tsx": REASON }, styles: {} });
  assert.deepEqual(checkDesktop(listed), []);
});

test("a stylesheet class named for the desktop fails until it is listed", () => {
  const css = app({ "src/styles/a.css": ".rm-desktop-banner { color: red; }" });
  assert.match(checkDesktop(css)[0], /\.rm-desktop-banner is for the desktop alone/);
  assert.deepEqual(checkDesktop(app({ "src/styles/a.css": ".rm-desktop-banner { color: red; }" }, { files: {}, styles: { "rm-desktop-banner": REASON } })), []);
});

test("a list that is stale, or gives no reason, fails", () => {
  const gone = app({}, { files: { "src/components/DesktopGone.tsx": REASON }, styles: {} });
  assert.match(checkDesktop(gone)[0], /which is not there/);
  const plain = app({ "src/web/Page.tsx": "export const a = 1;" }, { files: { "src/web/Page.tsx": REASON }, styles: {} });
  assert.match(checkDesktop(plain)[0], /no longer knows about the desktop/);
  const bare = app({ "src/components/DesktopThing.tsx": "x" }, { files: { "src/components/DesktopThing.tsx": "todo" }, styles: {} });
  assert.match(checkDesktop(bare)[0], /gives no reason/);
  const oldStyle = app({}, { files: {}, styles: { "rm-desktop-old": REASON } });
  assert.match(checkDesktop(oldStyle)[0], /no stylesheet has/);
});

test("tests and generated files are not the app's screens", () => {
  assert.deepEqual(checkDesktop(app({ "src/test/DesktopApp.test.tsx": 'import "@tauri-apps/api";', "src/generated/domain.ts": "export {};" })), []);
});

test("the real app passes with the list it has", () => {
  const real = join(dirname(fileURLToPath(import.meta.url)), "..", "apps", "desktop");
  assert.deepEqual(checkDesktop(real), []);
});
