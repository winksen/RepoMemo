#!/usr/bin/env node
// The troubleshooting page lists every error a start and the first-run setup can raise (V0.4.64). This finds those errors in the
// source and requires each to be quoted in docs/TROUBLESHOOTING.md, so a new error cannot ship without its cause and remedy.
//
//   node scripts/check-troubleshooting.mjs          (npm run troubleshooting:check; CI runs it with the other checks)
//   node scripts/check-troubleshooting.mjs --list   prints the errors it found

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

/** Where the errors of a start and of the first-run setup are raised. */
export const SOURCES = [
  "apps/server/src/config.rs",
  "apps/server/src/setup.rs",
  "apps/server/src/check_config.rs",
  "crates/storage/src/newer.rs",
  "docker/web-entrypoint.sh",
];

const MIN_FRAGMENT = 14;
const LITERAL = String.raw`"((?:[^"\\]|\\.)*)"`;

/** The text of a string literal before its first placeholder, with Rust's escapes undone. */
function staticPart(literal) {
  const text = literal.replace(/\\"/g, '"').replace(/\\n/g, " ").replace(/\\\\/g, "\\");
  // The longest piece between placeholders (`{name}`, `{}`): the words that stay the same whatever the value.
  return text.split(/\{[^}]*\}|\$\{?\w+\}?/).map((piece) => piece.trim()).sort((a, b) => b.length - a.length)[0] ?? "";
}

/** The fragments of error messages in some Rust source: what `bail!`, `with_context`, `anyhow!` and `ApiError::...` are given. */
export function errorFragments(source, { allFormats = false } = {}) {
  const found = new Set();
  const patterns = [
    new RegExp(String.raw`bail!\(\s*` + LITERAL, "g"),
    new RegExp(String.raw`(?:with_context|context)\(\s*\|\|\s*format!\(\s*` + LITERAL, "g"),
    new RegExp(String.raw`anyhow!\(\s*` + LITERAL, "g"),
    new RegExp(String.raw`ApiError::(?:conflict|not_found|bad_request|forbidden_because|unauthorized|too_many_requests)\(\s*` + LITERAL, "g"),
  ];
  // The web image's entrypoint refuses a setting by saying so on standard error.
  patterns.push(new RegExp(String.raw`echo\s+"repomemo-web:\s*` + String.raw`((?:[^"\\]|\\.)*)"\s*>&2\s*;?\s*exit 1`, "g"));
  // The folder check says what is wrong, or doubtful, by pushing a `format!` onto its errors or its warnings.
  if (allFormats) patterns.push(new RegExp(String.raw`(?:errors|warnings)\s*\.push\(\s*format!\(\s*` + LITERAL, "g"));
  for (const pattern of patterns) {
    for (const match of source.matchAll(pattern)) {
      const fragment = staticPart(match[1]);
      if (fragment.length >= MIN_FRAGMENT) found.add(fragment);
    }
  }
  return [...found].sort();
}

/** The fragments that the page does not quote. */
export function missingFragments(page, fragments) {
  const flat = page.replace(/\s+/g, " ");
  return fragments.filter((fragment) => !flat.includes(fragment.replace(/\s+/g, " ")));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const root = join(fileURLToPath(new URL("..", import.meta.url)));
  const all = SOURCES.flatMap((file) => errorFragments(readFileSync(join(root, file), "utf8"), { allFormats: file.endsWith("check_config.rs") }).map((fragment) => ({ file, fragment })));
  if (process.argv.includes("--list")) {
    for (const { file, fragment } of all) console.log(`${file}: ${fragment}`);
    process.exit(0);
  }
  const page = readFileSync(join(root, "docs", "TROUBLESHOOTING.md"), "utf8");
  const missing = missingFragments(page, all.map((entry) => entry.fragment));
  if (missing.length > 0) {
    console.error(`docs/TROUBLESHOOTING.md does not list ${missing.length} error${missing.length === 1 ? "" : "s"} of a start or the first-run setup:`);
    for (const fragment of missing) console.error(` - ${all.find((entry) => entry.fragment === fragment).file}: ${fragment}`);
    process.exit(1);
  }
  console.log(`Troubleshooting: all ${all.length} errors of a start and the first-run setup are listed.`);
}
