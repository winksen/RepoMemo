#!/usr/bin/env node
// Checks that the documentation still matches the code it describes (roadmap R11). It reads files
// only; nothing is started or built.
//
//   node scripts/check-docs.mjs      (npm run docs:check)
//
//   1. Every route of the server (node scripts/list-routes.mjs) has a row in
//      mindmap/technical/api-reference.md.
//   2. Every REPOMEMO_* variable the Rust code reads is described in docs/DEVELOPMENT_COMMANDS.md.
//   3. The version the mindmaps and the current release index (0.4) say they describe is the
//      product's version.
//   4. Every release of an index (0.2, 0.3, 0.4) marked done has an entry in CHANGELOG.md, and every
//      entry of the changelog from 0.2.1 on has a row in its index.
//   5. Every relative link of the documents points at a file that exists and, in a Markdown file,
//      at a heading that exists.

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8").replace(/\r\n/g, "\n");
const problems = [];
const version = JSON.parse(read("package.json")).version;

function* walk(directory, accept) {
  for (const entry of readdirSync(directory)) {
    if (["node_modules", "target", "dist", ".git", "workspace-data"].includes(entry)) continue;
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) yield* walk(path, accept);
    else if (accept(path)) yield path;
  }
}

// 1. Routes -----------------------------------------------------------------
const generated = execFileSync("node", [join(root, "scripts", "list-routes.mjs"), "--markdown"], { encoding: "utf8" });
const reference = read("mindmap/technical/api-reference.md");
let routes = 0;
for (const line of generated.split(/\r?\n/)) {
  const match = /^\|\s*(GET|POST|PUT|DELETE|PATCH)\s*\|\s*`([^`]+)`/.exec(line);
  if (!match) continue;
  routes += 1;
  const row = new RegExp(`^\\|\\s*${match[1]}\\s*\\|\\s*\`${match[2].replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\``, "m");
  if (!row.test(reference)) problems.push(`mindmap/technical/api-reference.md has no row for ${match[1]} ${match[2]}`);
}

// 2. Settings ---------------------------------------------------------------
const commands = read("docs/DEVELOPMENT_COMMANDS.md");
const variables = new Set();
for (const path of walk(join(root, "apps"), (file) => file.endsWith(".rs") && !/[\\/]tests?[\\/]/.test(file))) {
  for (const match of readFileSync(path, "utf8").matchAll(/"(REPOMEMO_[A-Z0-9_]+)"/g)) variables.add(match[1]);
}
for (const path of walk(join(root, "crates"), (file) => file.endsWith(".rs") && !/[\\/]tests?[\\/]/.test(file))) {
  for (const match of readFileSync(path, "utf8").matchAll(/"(REPOMEMO_[A-Z0-9_]+)"/g)) variables.add(match[1]);
}
for (const name of [...variables].sort()) {
  if (!commands.includes(name)) problems.push(`docs/DEVELOPMENT_COMMANDS.md does not describe ${name}, which the code reads`);
}

// 3. Versions ---------------------------------------------------------------
const stamps = [
  ["mindmap/FUNCTIONAL_MINDMAP.md", /reflects the code at `V([\d.]+)`/],
  ["mindmap/TECHNICAL_MINDMAP.md", /code at `V([\d.]+)`/],
  ["mindmap/ROADMAP.md", /\*\*Current version:\*\* `V([\d.]+)`/],
  ["mindmap/RELEASES_0.4.md", /\*\*Current:\*\* `V([\d.]+)`/],
];
for (const [path, pattern] of stamps) {
  const found = pattern.exec(read(path))?.[1];
  if (found !== version) problems.push(`${path} describes V${found ?? "?"}; the product is V${version}`);
}
if (!new RegExp(`^## V${version.replace(/\./g, "\\.")} \\(`, "m").test(read("CHANGELOG.md"))) problems.push(`CHANGELOG.md has no entry for V${version}`);

// 4. Releases ---------------------------------------------------------------
const changelog = read("CHANGELOG.md");
// One index for each minor line; the line's first number (0.2.0, 0.3.0, 0.4.0) is the previous line's last.
for (const [line, file] of [["2", "mindmap/RELEASES_0.2.md"], ["3", "mindmap/RELEASES_0.3.md"], ["4", "mindmap/RELEASES_0.4.md"]]) {
  const first = `0.${line}.0`;
  const entries = new Set([...changelog.matchAll(new RegExp(`^## V(0\\.${line}\\.\\d+) \\(`, "gm"))].map((match) => match[1]));
  const index = read(file);
  const done = new Set();
  for (const match of index.matchAll(new RegExp(`^\\| \\*\\*V(0\\.${line}\\.\\d+)\\*\\* \\| (✅|🔜)`, "gm"))) {
    if (match[2] === "✅") done.add(match[1]);
    if (match[2] === "✅" && !entries.has(match[1]) && match[1] !== first) problems.push(`${file} marks V${match[1]} done and CHANGELOG.md has no entry for it`);
  }
  for (const entry of entries) {
    if (entry !== first && !done.has(entry)) problems.push(`CHANGELOG.md has an entry for V${entry} and ${file} does not mark it done`);
  }
}

// 5. Links ------------------------------------------------------------------
function slugs(markdown) {
  const seen = new Map();
  const result = new Set();
  let fenced = false;
  for (const line of markdown.split("\n")) {
    if (/^```/.test(line)) fenced = !fenced;
    const heading = !fenced && /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (!heading) continue;
    const text = heading[2].replace(/`/g, "").replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
    let slug = text.toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, "").trim().replace(/\s/g, "-");
    // The emoji of a heading goes and leaves its space behind.
    if (/^[^\p{L}\p{N}\s_-]/u.test(text.toLowerCase()) && /^\s/.test(text.toLowerCase().replace(/^[^\p{L}\p{N}\s_-]+/u, ""))) slug = `-${slug}`;
    const count = seen.get(slug) ?? 0;
    seen.set(slug, count + 1);
    result.add(count ? `${slug}-${count}` : slug);
  }
  return result;
}
const slugCache = new Map();
const slugsOf = (path) => {
  if (!slugCache.has(path)) slugCache.set(path, slugs(readFileSync(path, "utf8").replace(/\r\n/g, "\n")));
  return slugCache.get(path);
};
const documents = [
  ...walk(join(root, "mindmap"), (file) => file.endsWith(".md") && !/[\\/]analysis[\\/]/.test(file)),
  ...["README.md", "CHANGELOG.md", "UPGRADING.md", "PRODUCT.md", join("docs", "README.md"), join("docs", "DEVELOPMENT_COMMANDS.md"), join("docs", "TEMPLATES.md")].map((file) => join(root, file)),
];
for (const document of documents) {
  let fenced = false;
  read(relative(root, document)).split("\n").forEach((line, index) => {
    if (/^```/.test(line)) fenced = !fenced;
    if (fenced) return;
    for (const match of line.replace(/`[^`]*`/g, "").matchAll(/\]\(([^)\s]+)\)/g)) {
      const target = match[1];
      if (/^(https?:|mailto:|#$)/.test(target)) continue;
      const [file, anchor] = target.split("#");
      const path = file ? resolve(dirname(document), decodeURIComponent(file)) : document;
      if (!existsSync(path)) {
        problems.push(`${relative(root, document)}:${index + 1}: the link ${target} points at a file that is not there`);
      } else if (anchor && path.endsWith(".md") && !slugsOf(path).has(anchor)) {
        problems.push(`${relative(root, document)}:${index + 1}: the link ${target} points at a heading that is not there`);
      }
    }
  });
}

if (problems.length) {
  for (const problem of problems) console.error(`✗ ${problem}`);
  console.error(`\n${problems.length} documentation problem${problems.length === 1 ? "" : "s"}.`);
  process.exit(1);
}
console.log(`Documentation: ${routes} routes, ${variables.size} settings, versions at V${version}, ${documents.length} documents' links all in order.`);
