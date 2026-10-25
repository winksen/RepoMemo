// The data of an earlier release must survive the next one (V0.4.27). Two phases against a server:
//
//   node scripts/upgrade-test.mjs seed   http://127.0.0.1:3020 <setup code> <state file>   on the EARLIER release (a new server)
//   node scripts/upgrade-test.mjs verify http://127.0.0.1:8080 <state file>                on the NEW release, same volumes
//
// `seed` makes the administrator, a workspace and an uploaded note, waits for its indexing, and writes what it made
// into the state file. `verify` signs the same administrator in and finds the workspace, the note and its text by search.
// Only the API calls that every release since 0.3.0 has are used, so the same script runs against both.
import { readFileSync, writeFileSync } from "node:fs";

const [phase, rawBase, third, fourth] = process.argv.slice(2);
const base = (rawBase ?? "").replace(/\/$/, "");
const email = "upgrade-test@example.com";
const password = "a-long-enough-upgrade-password-1";
const needle = "quokka protocol";
if (!["seed", "verify"].includes(phase) || !base) {
  console.error("usage: upgrade-test.mjs seed <base> <setup code> <state file> | verify <base> <state file>");
  process.exit(2);
}

let failed = 0;
const check = (name, ok, detail = "") => {
  if (!ok) failed += 1;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? ` - ${detail}` : ""}`);
};
const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
async function call(path, { method = "GET", token, body, headers = {} } = {}) {
  const response = await fetch(base + path, {
    method,
    headers: { ...(token ? { authorization: `Bearer ${token}` } : {}), ...(typeof body === "string" ? {} : body === undefined ? {} : { "content-type": "application/json" }), ...headers },
    body: body === undefined ? undefined : typeof body === "string" ? body : JSON.stringify(body),
  });
  const text = await response.text();
  let json = null;
  try { json = JSON.parse(text); } catch { /* not JSON */ }
  return { status: response.status, json, text };
}
async function searchFor(token, workspaceId) {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    const search = await call(`/v1/workspaces/${workspaceId}/search`, {
      method: "POST",
      token,
      body: { workspace_id: workspaceId, query: needle, artifact_types: [], languages: [], source_ids: [], limit: 5 },
    });
    if (search.text.includes("quokka")) return true;
    await sleep(1000);
  }
  return false;
}

if (phase === "seed") {
  const [code, stateFile] = [third, fourth];
  const created = await call("/v1/setup/admin", { method: "POST", body: { code, email, display_name: "Upgrade Test", password } });
  check("the earlier release makes its administrator", created.status === 201 && Boolean(created.json?.access_token), `status ${created.status}`);
  const token = created.json?.access_token;
  await call("/v1/setup/complete", { method: "POST", token });
  const organizations = await call("/v1/organizations", { token });
  let organization = Array.isArray(organizations.json) ? organizations.json[0] : null;
  if (!organization) organization = (await call("/v1/organizations", { method: "POST", token, body: { name: "Upgrade Organization" } })).json;
  const workspace = await call("/v1/workspaces", { method: "POST", token, body: { organization_id: organization?.id, name: "Upgrade Workspace" } });
  check("it makes a workspace", workspace.status === 201 && Boolean(workspace.json?.id), `status ${workspace.status}`);
  const workspaceId = workspace.json?.id;
  const note = "Upgrade note.\n\nThe quokka protocol keeps the data of an earlier release through the next one.\n";
  const uploaded = await call(`/v1/workspaces/${workspaceId}/artifacts/upload`, {
    method: "POST",
    token,
    body: note,
    headers: { "content-type": "text/markdown", "x-repomemo-filename": "upgrade-note.md" },
  });
  check("it takes an upload", uploaded.status === 201 || uploaded.status === 200, `status ${uploaded.status}`);
  check("it indexes the note", await searchFor(token, workspaceId));
  writeFileSync(stateFile, JSON.stringify({ workspaceId, artifactId: uploaded.json?.id ?? null, organizationId: organization?.id ?? null }));
} else {
  const state = JSON.parse(readFileSync(third, "utf8"));
  const login = await call("/v1/auth/login", { method: "POST", body: { email, password } });
  check("the new release signs the same administrator in", login.status === 200 && Boolean(login.json?.access_token), `status ${login.status}`);
  const token = login.json?.access_token;
  const workspaces = await call("/v1/workspaces", { token });
  check("the workspace is still there", Array.isArray(workspaces.json) && workspaces.json.some((item) => item.id === state.workspaceId), `status ${workspaces.status}`);
  const artifacts = await call(`/v1/workspaces/${state.workspaceId}/artifacts?limit=20&offset=0`, { token });
  check("the uploaded note is still there", artifacts.status === 200 && artifacts.text.includes("upgrade-note.md"), `status ${artifacts.status}`);
  if (state.artifactId) {
    const file = await call(`/v1/artifacts/${state.artifactId}/file`, { token });
    check("its file reads back whole", file.status === 200 && file.text.includes(needle), `status ${file.status}`);
  }
  check("a search still finds its text", await searchFor(token, state.workspaceId));
}

console.log(failed === 0 ? "Nothing was lost." : `${failed} check${failed === 1 ? "" : "s"} failed.`);
process.exit(failed === 0 ? 0 : 1);
