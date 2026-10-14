#!/usr/bin/env node
// Keeps the web app's stylesheets manageable (roadmap C11): no stylesheet over
// 1,000 lines, and `src/styles/index.ts` imports every file of `src/styles/`
// once, in the order of their numbers (the order is the cascade).
//
//   node scripts/check-styles.mjs     (npm run styles:check)
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "apps", "desktop", "src");
const LIMIT = 1000;
const problems = [];

function* stylesheets(directory) {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) yield* stylesheets(path);
    else if (entry.endsWith(".css")) yield path;
  }
}

for (const path of stylesheets(src)) {
  const lines = readFileSync(path, "utf8").split("\n").length;
  if (lines > LIMIT) problems.push(`${relative(root, path)} has ${lines} lines (limit ${LIMIT}); split it where a section starts`);
}

const stylesDir = join(src, "styles");
const files = readdirSync(stylesDir).filter((name) => /^\d\d-.*\.css$/.test(name)).sort();
const index = readFileSync(join(stylesDir, "index.ts"), "utf8");
const imported = [...index.matchAll(/^import "\.\/([^"]+\.css)";$/gm)].map((match) => match[1]);
if (JSON.stringify(imported) !== JSON.stringify(files)) {
  problems.push(`styles/index.ts must import ${files.join(", ")} in that order; it imports ${imported.join(", ") || "nothing"}`);
}

if (problems.length) {
  for (const problem of problems) console.error(`✗ ${problem}`);
  process.exit(1);
}
console.log(`Stylesheets are under ${LIMIT} lines and imported in order (${files.length} files in src/styles).`);
