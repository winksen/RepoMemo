// Tests of scripts/check-templates.mjs (V0.2.188), with the runner that comes with Node:
//
//   node --test scripts/check-templates.test.mjs      (npm run templates:test)
//
// Each case writes a template to a folder of its own and runs the checker on that one file.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const checker = join(dirname(fileURLToPath(import.meta.url)), "check-templates.mjs");
const folder = mkdtempSync(join(tmpdir(), "repomemo-templates-"));
after(() => rmSync(folder, { recursive: true, force: true }));

/** A template that is valid but for what `edit` changes. */
function template(id, edit = () => {}) {
  const value = { format: 1, meta: { id, version: 1, name: "Test", summary: "A test template", icon: "stack" } };
  edit(value);
  const path = join(folder, `${id}.json`);
  writeFileSync(path, JSON.stringify(value, null, 2));
  return path;
}
const run = (...files) => spawnSync(process.execPath, [checker, ...files], { encoding: "utf8" });

describe("the template checker", () => {
  it("passes the templates of the repository, the example and the code", () => {
    const result = run();
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Templates: \d+ files valid/);
  });

  it("passes a valid file", () => {
    const result = run(template("valid-one", (value) => (value.sections = { order: ["overview", "review"] })));
    assert.equal(result.status, 0, result.stderr);
  });

  it("names the file, the line and the column of a misspelled section id", () => {
    const result = run(template("misspelled", (value) => (value.sections = { order: ["overview", "evidnce"] })));
    assert.equal(result.status, 1);
    assert.match(result.stderr, /misspelled\.json:\d+:\d+ {2}sections\.order\[1\] {2}`evidnce` is not a workspace section/);
  });

  it("refuses a factor of 1.6", () => {
    const result = run(template("too-strong", (value) => (value.ranking = { verified: 1.6 })));
    assert.equal(result.status, 1);
    assert.match(result.stderr, /too-strong\.json:\d+:\d+ {2}ranking\.verified {2}the factor is 1\.6; it is from 0\.8 to 1\.3/);
  });

  it("refuses an unknown key, at the key", () => {
    const result = run(template("odd-key", (value) => (value.colour = "red")));
    assert.equal(result.status, 1);
    assert.match(result.stderr, /odd-key\.json:\d+:\d+ {2}colour {2}unknown key `colour`/);
  });

  it("refuses a pattern that looks like a regular expression, a headline it cannot count and a repeated entry", () => {
    const result = run(
      template("several", (value) => {
        value.repositories = { exclude: ["(a|b)", "*.log", "*.log"] };
        value.overview = { cards: ["sources"], headline: "index_coverage" };
      }),
    );
    assert.equal(result.status, 1);
    assert.match(result.stderr, /repositories\.exclude\[0\]/);
    assert.match(result.stderr, /repositories\.exclude\[2\] {2}`\*\.log` is listed twice/);
    assert.match(result.stderr, /overview\.headline/);
  });

  it("says where the JSON itself is wrong", () => {
    const path = join(folder, "broken.json");
    writeFileSync(path, '{\n  "format": 1,\n  "meta": {\n');
    const result = run(path);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /broken\.json:\d+:\d+ /);
  });

  it("wants a file named after its id", () => {
    const path = join(folder, "other-name.json");
    writeFileSync(path, JSON.stringify({ format: 1, meta: { id: "different", version: 1, name: "Test", summary: "A test template", icon: "stack" } }));
    const result = run(path);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /different\.json/);
  });
});
