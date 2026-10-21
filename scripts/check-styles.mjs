#!/usr/bin/env node
// Keeps the web app's stylesheets manageable (roadmap C11) and stops them drifting (track K):
//
//   - no stylesheet over 1,000 lines, and `src/styles/index.ts` imports every file of
//     `src/styles/` once, in the order of their numbers (the order is the cascade);
//   - no bare `input`, `select`, `textarea` or `button` selector (alone, or at the end of a path
//     such as `.page-form input`) outside the base reset (the top of 01-tokens-and-shared-web.css,
//     before its "Shared web mode" comment) and the retired local edition (`.rm-local-edition`):
//     a control is styled through its class (`.rm-input`, `.rm-button`…), never from a page;
//   - no hex, rgb or hsl colour outside a `:root` or `[data-theme]` token block;
//   - no `z-index` that is not a `var(--layer-*)` token (0, -1 and auto are allowed);
//   - no `height` or `min-height` in pixels on a button, input, select or dropdown trigger:
//     controls take `var(--control-height)` and its siblings.
//
// What the tree already did before these rules is listed in `scripts/styles-baseline.json`. The
// check fails for anything not in that list (a new violation) and for a listed entry that is gone
// (remove it, so the list only shrinks).
//
//   node scripts/check-styles.mjs                          (npm run styles:check)
//   node scripts/check-styles.mjs --update-baseline        rewrite the list after a cleanup
//   node scripts/check-styles.mjs --dir <folder> --baseline <file>    check another folder (the tests do)
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const option = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const src = join(root, "apps", "desktop", "src");
const stylesDir = option("--dir") ?? join(src, "styles");
const baselinePath = option("--baseline") ?? join(root, "scripts", "styles-baseline.json");
const updateBaseline = args.includes("--update-baseline");
const LIMIT = 1000;
const problems = [];

function* stylesheets(directory) {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) yield* stylesheets(path);
    else if (entry.endsWith(".css")) yield path;
  }
}

for (const path of stylesheets(option("--dir") ?? src)) {
  const lines = readFileSync(path, "utf8").split("\n").length;
  if (lines > LIMIT) problems.push(`${relative(root, path)} has ${lines} lines (limit ${LIMIT}); split it where a section starts`);
}

const files = readdirSync(stylesDir).filter((name) => /^\d\d-.*\.css$/.test(name)).sort();
const index = readFileSync(join(stylesDir, "index.ts"), "utf8");
const imported = [...index.matchAll(/^import "\.\/([^"]+\.css)";$/gm)].map((match) => match[1]);
if (JSON.stringify(imported) !== JSON.stringify(files)) {
  problems.push(`styles/index.ts must import ${files.join(", ")} in that order; it imports ${imported.join(", ") || "nothing"}`);
}

/** The rules of a stylesheet as `{ head, body, line, parents }`; at-rules are looked through. */
function rules(text) {
  const clean = text.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, " "));
  const found = [];
  const stack = [];
  let buffer = "";
  let line = 1;
  let startLine = 1;
  for (const character of clean) {
    if (character === "\n") line += 1;
    if (character === "{") {
      stack.push({ head: buffer.trim(), line: startLine, body: "" });
      buffer = "";
      continue;
    }
    if (character === "}") {
      const top = stack.pop();
      if (top) {
        top.body += buffer;
        found.push({ ...top, parents: stack.map((entry) => entry.head) });
      }
      buffer = "";
      startLine = line;
      continue;
    }
    if (buffer.trim() === "") startLine = line;
    buffer += character;
    if (character === ";" && stack.length) {
      stack[stack.length - 1].body += buffer;
      buffer = "";
    }
  }
  return found;
}

const BARE = /^(input|select|textarea|button)(:[\w-]+(\([^)]*\))?|\[[^\]]*\])*$/;
const CONTROL_CLASS = /(^|[-_.])(button|btn|input|select|trigger)$/;
const TOKEN_BLOCK = /^(html)?:root\b|^(html)?\[data-theme/;
const COLOUR = /#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?)\([^)]*\)/g;

/** Every way a stylesheet breaks the rules above, as lines that name the file and the selector. */
function violationsIn(file, text) {
  const found = [];
  const resetEnd = file.startsWith("01-") ? text.search(/Shared web mode/) : -1;
  const resetLines = resetEnd < 0 ? 0 : text.slice(0, resetEnd).split("\n").length;
  for (const rule of rules(text)) {
    if (rule.head.startsWith("@") || rule.parents.some((parent) => /^@(keyframes|font-face)/.test(parent))) continue;
    const selectors = rule.head.split(",").map((selector) => selector.replace(/\s+/g, " ").trim()).filter(Boolean);
    const inReset = rule.line < resetLines;
    const local = rule.head.includes(".rm-local-edition");
    const add = (kind, selector, detail) => found.push(`${kind} | ${file} | ${selector} | ${detail}`);

    if (!local) {
      for (const selector of selectors) {
        const last = selector.split(/[\s>+~]+/).filter(Boolean).pop() ?? "";
        const pure = !/[\s>+~]/.test(selector);
        if (BARE.test(last) && !(inReset && pure)) add("bare element", selector, "style it through its class");
        if (inReset) continue;
        const isControl = BARE.test(last) ? !last.startsWith("textarea") : (last.match(/\.[\w-]+/g) ?? []).some((name) => CONTROL_CLASS.test(name));
        if (!isControl) continue;
        for (const match of rule.body.matchAll(/(?:^|[;\s])((?:min-)?height)\s*:\s*(\d+px)\s*(?=;|$)/g)) add("control height", selector, `${match[1]}: ${match[2]}`);
      }
    }
    if (!selectors.every((selector) => TOKEN_BLOCK.test(selector))) {
      for (const match of rule.body.matchAll(COLOUR)) add("raw colour", selectors.join(", "), match[0]);
    }
    for (const match of rule.body.matchAll(/z-index\s*:\s*([^;]+)/g)) {
      const value = match[1].trim();
      if (!/^var\(--layer-[\w-]+\)$/.test(value) && !/^(0|-1|auto)$/.test(value)) add("raw layer", selectors.join(", "), `z-index: ${value}`);
    }
  }
  return found;
}

// The rules must keep catching what they exist for: a planted stylesheet has to trip each of them,
// and its clean twin none.
const PLANTED = [".page-form input { height: 40px; }", ".card { color: #ff0000; z-index: 5; }", ".rm-button.mine { min-height: 30px; }"].join(" ");
const CLEAN = ":root { --ok: #123456; } .fine { color: var(--color-text); z-index: var(--layer-raised); min-height: var(--control-height); } .rm-local-edition input { height: 38px; }";
const caught = new Set(violationsIn("01-planted.css", PLANTED).map((violation) => violation.split(" | ")[0]));
for (const kind of ["bare element", "raw colour", "raw layer", "control height"]) {
  if (!caught.has(kind)) problems.push(`the style check no longer catches a planted ${kind}`);
}
if (violationsIn("02-clean.css", CLEAN).length) problems.push("the style check refuses a clean stylesheet");

const violations = files.flatMap((file) => violationsIn(file, readFileSync(join(stylesDir, file), "utf8"))).sort();

if (updateBaseline) {
  writeFileSync(baselinePath, `${JSON.stringify(violations, null, 2)}\n`);
  console.log(`Wrote ${violations.length} entries to ${relative(root, baselinePath)}.`);
  process.exit(0);
}

const baseline = existsSync(baselinePath) ? JSON.parse(readFileSync(baselinePath, "utf8")) : [];
const allowed = new Map();
for (const entry of baseline) allowed.set(entry, (allowed.get(entry) ?? 0) + 1);
for (const violation of violations) {
  const left = allowed.get(violation) ?? 0;
  if (left > 0) allowed.set(violation, left - 1);
  else {
    const [kind, file, selector, detail] = violation.split(" | ");
    problems.push(`${file}: ${kind} in \`${selector}\` (${detail})`);
  }
}
for (const [entry, left] of allowed) {
  if (left > 0) problems.push(`gone from the styles, remove it from scripts/styles-baseline.json (npm run styles:check -- --update-baseline): ${entry}`);
}

if (problems.length) {
  for (const problem of problems) console.error(`✗ ${problem}`);
  process.exit(1);
}
console.log(`Stylesheets are under ${LIMIT} lines and imported in order (${files.length} files in src/styles); no new bare element style, raw colour, raw layer or raw control height (${baseline.length} older ones are listed in scripts/styles-baseline.json).`);
