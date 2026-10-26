#!/usr/bin/env node
// The parity check of the desktop app (V0.4.58): the app is the web client in a window, so nothing may exist for the desktop alone
// without being listed, with its reason, in apps/desktop/desktop-only.json.
//
//   node scripts/check-desktop.mjs          (npm run desktop:check; CI runs it with the other checks)
//
// A file is desktop-only when it knows about the shell (its name, an import of the desktop module, a Tauri package, or the runtime
// test) and a style is when a class is named for the desktop. Both must be listed; a listed entry that no longer exists, or that gives no
// reason, fails too, so the list cannot grow stale. It reads files only.

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const MARKERS = /from "(?:\.\.?\/)+(?:lib\/)?desktop"|@tauri-apps|isDesktopRuntime|__TAURI_INTERNALS__|installExternalLinkHandler|desktopSession/;
const MIN_REASON = 12;

function filesUnder(folder, extensions) {
  const found = [];
  const walk = (current) => {
    for (const name of readdirSync(current).sort()) {
      const path = join(current, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (extensions.some((extension) => name.endsWith(extension))) found.push(path);
    }
  };
  if (existsSync(folder)) walk(folder);
  return found;
}

/** The problems of an app folder (`apps/desktop`) against its list; an empty array is a pass. */
export function checkDesktop(appRoot) {
  const problems = [];
  const listPath = join(appRoot, "desktop-only.json");
  if (!existsSync(listPath)) return [`${listPath} is missing`];
  const list = JSON.parse(readFileSync(listPath, "utf8"));
  const files = list.files ?? {};
  const styles = list.styles ?? {};
  const source = join(appRoot, "src");

  const desktopOnly = new Set();
  for (const path of filesUnder(source, [".ts", ".tsx"])) {
    const name = relative(appRoot, path).split("\\").join("/");
    if (name.startsWith("src/test/") || name.startsWith("src/generated/")) continue;
    if (/desktop/i.test(name.split("/").pop()) || MARKERS.test(readFileSync(path, "utf8"))) desktopOnly.add(name);
  }
  for (const name of desktopOnly) {
    if (!(name in files)) problems.push(`${name} exists for the desktop alone but is not in desktop-only.json: make it part of the web client, or list it with the reason it is not`);
  }
  for (const [name, reason] of Object.entries(files)) {
    if (!existsSync(join(appRoot, name))) problems.push(`desktop-only.json lists ${name}, which is not there; remove it`);
    else if (!desktopOnly.has(name)) problems.push(`desktop-only.json lists ${name}, which no longer knows about the desktop; remove it`);
    if (typeof reason !== "string" || reason.trim().length < MIN_REASON) problems.push(`desktop-only.json gives no reason for ${name}`);
  }

  const classes = new Set();
  for (const path of filesUnder(join(source, "styles"), [".css"])) {
    for (const match of readFileSync(path, "utf8").matchAll(/\.([A-Za-z0-9_-]*desktop[A-Za-z0-9_-]*)/gi)) classes.add(match[1]);
  }
  for (const name of classes) {
    if (!(name in styles)) problems.push(`the style .${name} is for the desktop alone but is not in desktop-only.json`);
  }
  for (const [name, reason] of Object.entries(styles)) {
    if (!classes.has(name)) problems.push(`desktop-only.json lists the style .${name}, which no stylesheet has; remove it`);
    if (typeof reason !== "string" || reason.trim().length < MIN_REASON) problems.push(`desktop-only.json gives no reason for the style .${name}`);
  }
  return problems;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const root = join(fileURLToPath(new URL("..", import.meta.url)), "apps", "desktop");
  const problems = checkDesktop(root);
  if (problems.length > 0) {
    console.error(`The desktop app differs from the web client in ${problems.length} way${problems.length === 1 ? "" : "s"} not on the list:`);
    for (const problem of problems) console.error(` - ${problem}`);
    process.exit(1);
  }
  const list = JSON.parse(readFileSync(join(root, "desktop-only.json"), "utf8"));
  console.log(`Desktop parity: ${Object.keys(list.files).length} desktop-only files and ${Object.keys(list.styles).length} styles, each listed with its reason.`);
}
