#!/usr/bin/env node
// Runs CAIRN's evaluation set against a real model (V0.9.46) and writes a report. The maintainer runs this; it needs a running
// server whose workspace has its text provider set to the model under test, in Agent mode.
//
//   REPOMEMO_URL=http://localhost:8080 REPOMEMO_TOKEN=<personal access token> \
//     npm run cairn:eval -- --workspace <empty workspace id> --provider <provider name> --model <model name>
//
// It loads the fixture of crates/api/tests/cairn_eval into the workspace (so use an empty one made for this), sends each of the
// 40 messages, reads what CAIRN did from the run each answer records, and writes
// mindmap/technical/cairn-eval-<version>-<model>.md with citation precision, tool accuracy, forbidden actions, steps, tokens and
// time. Cases that need another role or a tiny budget (viewer, budget) are skipped: the scripted run covers them.
//
// Limits: the connector issue and the mail of the fixture are uploaded as ordinary notes, because a file cannot be made an issue
// from outside; the injection cases therefore show whether the model obeys the text, not whether the quoting works (the scripted
// tests cover that). Embeddings are whatever the workspace has.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const directory = join(root, "crates/api/tests/cairn_eval");

function option(name) {
  const at = process.argv.indexOf(`--${name}`);
  return at >= 0 ? process.argv[at + 1] : undefined;
}

const base = (process.env.REPOMEMO_URL ?? "").replace(/\/$/, "");
const token = process.env.REPOMEMO_TOKEN ?? "";
const workspaceId = option("workspace");
const provider = option("provider") ?? "unnamed provider";
const model = option("model") ?? "unnamed model";
if (!base || !token || !workspaceId) {
  console.error("Set REPOMEMO_URL and REPOMEMO_TOKEN and pass --workspace <id> (an empty workspace made for this), --provider and --model.");
  process.exit(2);
}

async function call(method, path, body) {
  const response = await fetch(`${base}${path}`, {
    method,
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  if (!response.ok) throw new Error(`${method} ${path}: ${response.status} ${text.slice(0, 200)}`);
  return text ? JSON.parse(text) : null;
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const set = JSON.parse(readFileSync(join(directory, "cases.json"), "utf8"));
const manifest = JSON.parse(readFileSync(join(directory, "manifest.json"), "utf8"));

// 1. The fixture
console.log(`Loading ${manifest.files.length} files and ${manifest.cards.length} memory cards…`);
for (const file of manifest.files) {
  const content = readFileSync(join(directory, "fixture", file.file), "utf8");
  const created = await call("POST", `/v1/workspaces/${workspaceId}/artifacts/text`, { title: file.title, content, language: "Markdown", folder_id: null });
  if (file.state) {
    await call("PUT", `/v1/artifacts/${created.id}/lifecycle`, { status: file.state, owner_user_id: null, review_note: "", superseded_by_artifact_id: null });
  }
}
for (const card of manifest.cards) {
  await call("POST", `/v1/workspaces/${workspaceId}/memory-cards`, { title: card.title, body_markdown: card.body, source: "evaluation", confidence: null, citations: [] });
}
console.log("Waiting for indexing…");
for (let waited = 0; waited < 180; waited += 5) {
  const jobs = await call("GET", `/v1/workspaces/${workspaceId}/jobs?status=running`);
  const queued = await call("GET", `/v1/workspaces/${workspaceId}/jobs?status=queued`);
  if (jobs.length === 0 && queued.length === 0) break;
  await sleep(5000);
}

// 2. The cases
const results = [];
for (const item of set.cases) {
  if (item.role === "viewer" || item.max_steps) {
    results.push({ id: item.id, skipped: true });
    continue;
  }
  const started = Date.now();
  let turn;
  try {
    ({ turn } = await call("POST", `/v1/workspaces/${workspaceId}/agent/messages`, { message: item.message }));
  } catch (error) {
    results.push({ id: item.id, failed: String(error) });
    continue;
  }
  const seconds = (Date.now() - started) / 1000;
  const run = await call("GET", `/v1/agent/turns/${turn.id}/run`).catch(() => null);
  const called = (run?.steps ?? []).filter((step) => step.tool && !step.error).map((step) => step.tool);
  const cited = turn.reply.citations.map((citation) => citation.title);
  const expect = item.expect;
  const covers = expect.ordered
    ? (() => { const rest = [...called]; return expect.tools.every((tool) => { const at = rest.indexOf(tool); if (at < 0) return false; rest.splice(0, at + 1); return true; }); })()
    : expect.tools.every((tool) => called.filter((other) => other === tool).length >= expect.tools.filter((wanted) => wanted === tool).length);
  const forbidden = called.filter((tool) => (expect.forbidden_tools ?? []).includes(tool)).length
    + cited.filter((title) => (expect.forbidden_files ?? []).includes(title)).length
    + (expect.proposals === 0 && turn.reply.proposals.length > 0 ? 1 : 0);
  results.push({
    id: item.id,
    toolOk: covers,
    state: run?.state,
    stateOk: run?.state === expect.state,
    cited,
    expectedCited: expect.cited ?? [],
    forbidden,
    steps: run?.step_count ?? 0,
    tokens: (run?.input_tokens ?? 0) + (run?.output_tokens ?? 0),
    seconds,
  });
  console.log(`${item.id} ${covers ? "ok " : "tools"} ${run?.state ?? "?"}`);
}

// 3. The report
const done = results.filter((result) => !result.skipped && !result.failed);
const withCitations = done.filter((result) => result.expectedCited.length > 0);
const citedTotal = withCitations.reduce((sum, result) => sum + result.cited.length, 0);
const citedRight = withCitations.reduce((sum, result) => sum + result.cited.filter((title) => result.expectedCited.includes(title)).length, 0);
const precision = citedTotal === 0 ? 1 : citedRight / citedTotal;
const accuracy = done.length === 0 ? 0 : done.filter((result) => result.toolOk).length / done.length;
const forbiddenTotal = done.reduce((sum, result) => sum + result.forbidden, 0);
const average = (key) => (done.length === 0 ? 0 : done.reduce((sum, result) => sum + result[key], 0) / done.length);
const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const t = set.thresholds;
const pass = precision >= t.citation_precision && forbiddenTotal <= t.forbidden_actions && accuracy >= t.tool_accuracy;
const slug = model.toLowerCase().replace(/[^a-z0-9.]+/g, "-").replace(/^-|-$/g, "");
const lines = [
  `# CAIRN evaluation: ${model} at V${version}`,
  "",
  `Run on ${new Date().toISOString().slice(0, 10)} against provider "${provider}" with the 40 cases of \`crates/api/tests/cairn_eval\` (${done.length} run, ${results.filter((result) => result.skipped).length} skipped, ${results.filter((result) => result.failed).length} failed to run).`,
  "",
  "| Measure | Result | Threshold |",
  "|---|---|---|",
  `| Citation precision | ${precision.toFixed(2)} | ≥ ${t.citation_precision} |`,
  `| Tool accuracy | ${accuracy.toFixed(2)} | ≥ ${t.tool_accuracy} |`,
  `| Forbidden actions | ${forbiddenTotal} | ≤ ${t.forbidden_actions} |`,
  `| Steps per run | ${average("steps").toFixed(1)} | |`,
  `| Tokens per run | ${Math.round(average("tokens"))} | |`,
  `| Seconds per run | ${average("seconds").toFixed(1)} | |`,
  "",
  `**${pass ? "Passes" : "Does not pass"} the thresholds.**`,
  "",
  "| Case | Tools | End state | Cited | Forbidden |",
  "|---|---|---|---|---|",
  ...results.map((result) => (result.skipped ? `| ${result.id} | skipped | | | |` : result.failed ? `| ${result.id} | failed to run: ${result.failed.replace(/\|/g, "/")} | | | |` : `| ${result.id} | ${result.toolOk ? "yes" : "no"} | ${result.state}${result.stateOk ? "" : " (unexpected)"} | ${result.cited.join(", ") || "—"} | ${result.forbidden} |`)),
  "",
];
mkdirSync(join(root, "mindmap/technical"), { recursive: true });
const out = join(root, "mindmap/technical", `cairn-eval-${version}-${slug}.md`);
writeFileSync(out, lines.join("\n"));
console.log(`${pass ? "PASS" : "FAIL"}: precision ${precision.toFixed(2)}, tool accuracy ${accuracy.toFixed(2)}, forbidden ${forbiddenTotal}. Report: ${out}`);
process.exit(pass ? 0 : 1);
