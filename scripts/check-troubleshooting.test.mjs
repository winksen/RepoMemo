// node --test scripts/check-troubleshooting.test.mjs  (npm run troubleshooting:test)
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { SOURCES, errorFragments, missingFragments } from "./check-troubleshooting.mjs";

test("it finds what bail!, with_context, anyhow! and ApiError are given, up to the words that do not change", () => {
  const source = `
    bail!("REPOMEMO_JWT_SECRET must contain at least 32 characters");
    bail!("{name} must be true or false, not {raw}");
    .with_context(|| format!("REPOMEMO_WEB_DIR ({}) could not be read", dir.display()))?;
    return Err(ApiError::conflict("This server is already set up."));
    bail!("short");
  `;
  const found = errorFragments(source);
  assert.deepEqual(found, ["REPOMEMO_JWT_SECRET must contain at least 32 characters", "This server is already set up.", "must be true or false, not", ") could not be read"].sort());
});

test("the folder check's findings are the ones pushed onto its errors and its warnings", () => {
  const source = `report.errors.push(format!("{name} {} is full: {} MiB free. Free some room or mount a larger volume.", a, b)); report.lines.push(format!("{label} {} (exists, writable)", d));`;
  assert.deepEqual(errorFragments(source, { allFormats: true }), ["MiB free. Free some room or mount a larger volume."]);
  assert.deepEqual(errorFragments(source), []);
});

test("an error the page does not quote is reported, whatever the spacing", () => {
  const page = "It says `This server is   already set up.` and more";
  assert.deepEqual(missingFragments(page, ["This server is already set up.", "The setup code is not correct."]), ["The setup code is not correct."]);
});

test("the real sources have errors to list, and the real page lists them all", () => {
  for (const file of SOURCES) {
    const text = readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
    assert.ok(errorFragments(text, { allFormats: file.endsWith("check_config.rs") }).length > 0, file);
  }
  const page = readFileSync(new URL("../docs/TROUBLESHOOTING.md", import.meta.url), "utf8");
  const all = SOURCES.flatMap((file) => errorFragments(readFileSync(new URL(`../${file}`, import.meta.url), "utf8"), { allFormats: file.endsWith("check_config.rs") }));
  assert.deepEqual(missingFragments(page, all), []);
});
