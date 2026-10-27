// A check of a running RepoMemo server, for a clean-machine install (the
// production guide, V0.1.76 R6). Nothing in it changes data except an optional
// backup. Usage:
//
//   node scripts/smoke-test.mjs https://memo.example.com
//
// With REPOMEMO_SMOKE_EMAIL and REPOMEMO_SMOKE_PASSWORD (a system
// administrator) it also signs in, reads the overview, the metrics, the
// backups and the mail settings, and makes a backup when REPOMEMO_SMOKE_BACKUP=1.
// For the clean-machine install of V0.2.x (mindmap/technical/clean-install.md):
//
//   REPOMEMO_SMOKE_MAIL=you@example.com     sends a test mail there and waits for it to leave the outbox
//   REPOMEMO_SMOKE_REMOTE_REPO=https://...  checks that a remote repository can be linked (nothing is stored);
//                                           needs REPOMEMO_SMOKE_WORKSPACE (a workspace id), and
//                                           REPOMEMO_SMOKE_REMOTE_TOKEN for a private one
//
// With REPOMEMO_SMOKE_SETUP_CODE (V0.4.26), on a NEW server whose REPOMEMO_SETUP_CODE is that value, it does the whole
// first hour: the first-run setup, a workspace, an upload, indexing, a search that finds it, a backup, an export and an
// import of that export. It makes an administrator (REPOMEMO_SMOKE_EMAIL and REPOMEMO_SMOKE_PASSWORD, or its own) and
// a workspace on the server it is given, so it is for a test stack, never a server with data.
const base = (process.argv[2] ?? "http://127.0.0.1:3020").replace(/\/$/, "");
const results = [];
const check = (name, ok, detail = "") => {
  results.push(ok);
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? ` - ${detail}` : ""}`);
};

async function call(path, options = {}) {
  try {
    const response = await fetch(base + path, { redirect: "manual", ...options });
    const text = await response.text();
    let json = null;
    try { json = JSON.parse(text); } catch { /* not JSON */ }
    return { status: response.status, headers: response.headers, text, json };
  } catch (error) {
    return { status: 0, headers: new Headers(), text: String(error), json: null };
  }
}

const setupCode = process.env.REPOMEMO_SMOKE_SETUP_CODE;
const jsonHeaders = { "content-type": "application/json" };
const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
if (setupCode) {
  process.env.REPOMEMO_SMOKE_EMAIL ??= "smoke-admin@example.com";
  process.env.REPOMEMO_SMOKE_PASSWORD ??= "a-long-enough-smoke-password-1";
}

const health = await call("/health");
check("GET /health answers 200", health.status === 200, `status ${health.status}`);
check("the service reports ok", health.json?.status === "ok");
const ready = await call("/health/ready");
check("GET /health/ready answers 200", ready.status === 200, ready.text.slice(0, 80));
let setupSession = null;
if (setupCode && health.json?.setup_required === true) {
  const created = await call("/v1/setup/admin", {
    method: "POST",
    headers: jsonHeaders,
    body: JSON.stringify({ code: setupCode, email: process.env.REPOMEMO_SMOKE_EMAIL, display_name: "Smoke Administrator", password: process.env.REPOMEMO_SMOKE_PASSWORD }),
  });
  check("the first-run setup makes the administrator", created.status === 201 && Boolean(created.json?.access_token), `status ${created.status} ${created.json?.error?.message ?? ""}`);
  setupSession = created.json?.access_token ?? null;
  if (setupSession) {
    const completed = await call("/v1/setup/complete", { method: "POST", headers: { authorization: `Bearer ${setupSession}` } });
    check("the first-run setup completes", completed.status === 200, `status ${completed.status}`);
  }
  const after = await call("/health");
  health.json = after.json;
}
check("setup is finished", health.json?.setup_required === false, health.json?.setup_required ? "open the web app and finish the first-run setup" : "");

const anonymous = await call("/v1/workspaces");
check("the API refuses anonymous callers", anonymous.status === 401, `status ${anonymous.status}`);
check("security headers are present", anonymous.headers.get("x-content-type-options") === "nosniff" && anonymous.headers.get("referrer-policy") === "no-referrer");
if (base.startsWith("https://")) {
  check("HSTS is sent over HTTPS", Boolean(health.headers.get("strict-transport-security")), "set REPOMEMO_PUBLIC_URL");
}
const page = await call("/");
check("the web app is served from the same host", page.status === 200 && page.text.includes("<div id=\"root\""), `status ${page.status}`);
check("the web app carries a content security policy", /Content-Security-Policy/i.test(page.text) || Boolean(page.headers.get("content-security-policy")));
const metrics = await call("/v1/system/metrics");
check("metrics need authentication", metrics.status === 401);
// Since V0.2.52 the public endpoints take small bodies, so a proxy that lets a large one through is not the reason it is refused.
const large = await call("/v1/auth/login", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ email: "smoke@example.com", password: "x".repeat(200 * 1024) }),
});
check("sign-in refuses a body of 200 KiB", large.status === 413, `status ${large.status}`);
const hook = await call("/v1/webhooks/repositories/smoke-test", { method: "POST", headers: { "x-gitlab-token": "guess" }, body: "{}" });
check("a webhook that cannot be verified answers 401", hook.status === 401, `status ${hook.status}`);

const email = process.env.REPOMEMO_SMOKE_EMAIL;
const password = process.env.REPOMEMO_SMOKE_PASSWORD;
if (email && password) {
  const login = await call("/v1/auth/login", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ email, password }),
  });
  check("an administrator signs in", login.status === 200 && Boolean(login.json?.access_token), `status ${login.status}`);
  const auth = { authorization: `Bearer ${login.json?.access_token}` };
  const overview = await call("/v1/system/overview", { headers: auth });
  check("System overview answers", overview.status === 200);
  if (overview.json) {
    console.log(`     version ${overview.json.instance.version}, free disk ${overview.json.storage.disk_free_bytes ?? "unknown"} bytes`);
  }
  const text = await call("/v1/system/metrics", { headers: auth });
  check("metrics answer in OpenMetrics text", text.status === 200 && text.text.trimEnd().endsWith("# EOF"));
  const backups = await call("/v1/system/backups", { headers: auth });
  check("backups can be listed", backups.status === 200);
  const mail = await call("/v1/system/mail", { headers: auth });
  check("mail settings can be read", mail.status === 200);
  if (mail.json) {
    const settings = mail.json.settings;
    console.log(`     mail: ${settings.configured ? `${settings.host}:${settings.port} (${settings.security})` : "not set up"}, sending ${settings.enabled ? "on" : "off"}, ${mail.json.queued_count} waiting`);
  }
  const testAddress = process.env.REPOMEMO_SMOKE_MAIL;
  if (testAddress) {
    const queued = await call("/v1/system/mail/test", { method: "POST", headers: { ...auth, "content-type": "application/json" }, body: JSON.stringify({ to: testAddress }) });
    check("a test mail is queued", queued.status === 200 || queued.status === 201 || queued.status === 202, `status ${queued.status} ${queued.json?.error?.message ?? ""}`);
    let entry = queued.json;
    for (let attempt = 0; attempt < 60 && entry?.id && !["sent", "failed"].includes(entry.status); attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const outbox = await call("/v1/system/mail/outbox?limit=20", { headers: auth });
      entry = outbox.json?.find?.((item) => item.id === queued.json.id) ?? entry;
    }
    check(`the test mail to ${testAddress} is sent`, entry?.status === "sent", entry?.last_error ?? entry?.status ?? "no answer");
  }
  const remote = process.env.REPOMEMO_SMOKE_REMOTE_REPO;
  const workspace = process.env.REPOMEMO_SMOKE_WORKSPACE;
  if (remote && workspace) {
    const checked = await call(`/v1/workspaces/${workspace}/repositories/check`, {
      method: "POST",
      headers: { ...auth, "content-type": "application/json" },
      body: JSON.stringify({ link: remote, ...(process.env.REPOMEMO_SMOKE_REMOTE_TOKEN ? { token: process.env.REPOMEMO_SMOKE_REMOTE_TOKEN } : {}) }),
    });
    check("a remote repository can be linked (checked, nothing stored)", checked.status === 200, checked.json?.error?.message ?? `status ${checked.status}`);
    if (checked.json?.indexable_files !== undefined) console.log(`     ${checked.json.name}: ${checked.json.indexable_files} of ${checked.json.tracked_files} files would be indexed`);
  } else if (remote) {
    check("a workspace id is given for the remote repository check", false, "set REPOMEMO_SMOKE_WORKSPACE");
  }
  if (process.env.REPOMEMO_SMOKE_BACKUP === "1") {
    const started = await call("/v1/system/backups", { method: "POST", headers: { ...auth, "content-type": "application/json" }, body: "{}" });
    check("a backup starts", started.status === 202, `status ${started.status}`);
    let finished = null;
    for (let attempt = 0; attempt < 120 && !finished; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const state = await call("/v1/system/backups", { headers: auth });
      if (state.json && state.json.status.running === false && state.json.status.succeeded !== null) finished = state.json;
    }
    check("the backup finishes", finished?.status.succeeded === true, finished?.status.message ?? "timed out");
  }
}

// The first hour (V0.4.26): a workspace, an upload, indexing, search, a backup, an export and an import.
if (setupCode && email && password) {
  const login = await call("/v1/auth/login", { method: "POST", headers: jsonHeaders, body: JSON.stringify({ email, password }) });
  const auth = { authorization: `Bearer ${login.json?.access_token}` };
  const jsonAuth = { ...auth, ...jsonHeaders };
  const organizations = await call("/v1/organizations", { headers: auth });
  let organization = Array.isArray(organizations.json) ? organizations.json[0] : null;
  if (!organization) {
    const made = await call("/v1/organizations", { method: "POST", headers: jsonAuth, body: JSON.stringify({ name: "Smoke Organization" }) });
    organization = made.json;
  }
  check("an organization exists", Boolean(organization?.id), `status ${organizations.status}`);
  const workspace = await call("/v1/workspaces", { method: "POST", headers: jsonAuth, body: JSON.stringify({ organization_id: organization?.id, name: "Smoke Workspace" }) });
  const workspaceId = workspace.json?.workspace?.id;
  check("a workspace is made", workspace.status === 201 && Boolean(workspaceId), `status ${workspace.status} ${workspace.json?.error?.message ?? ""}`);
  if (workspaceId) {
    const note = "Deployment smoke note.\n\nThe quokka protocol keeps the containers honest: every release is started, used, backed up and restored.\n";
    const uploaded = await call(`/v1/workspaces/${workspaceId}/artifacts/upload`, {
      method: "POST",
      headers: { ...auth, "content-type": "text/markdown", "x-repomemo-filename": "smoke-note.md" },
      body: note,
    });
    check("a file is uploaded", uploaded.status === 201 || uploaded.status === 200, `status ${uploaded.status} ${uploaded.json?.error?.message ?? ""}`);
    let pending = true;
    for (let attempt = 0; attempt < 90 && pending; attempt += 1) {
      const jobs = await call(`/v1/workspaces/${workspaceId}/jobs?limit=20`, { headers: auth });
      pending = !Array.isArray(jobs.json) || jobs.json.some((job) => job.status === "pending" || job.status === "running") || jobs.json.length === 0;
      if (pending) await sleep(1000);
    }
    check("the file is indexed", !pending, pending ? "the jobs never finished" : "");
    let found = false;
    for (let attempt = 0; attempt < 30 && !found; attempt += 1) {
      const search = await call(`/v1/workspaces/${workspaceId}/search`, {
        method: "POST",
        headers: jsonAuth,
        body: JSON.stringify({ workspace_id: workspaceId, query: "quokka protocol", artifact_types: [], languages: [], source_ids: [], limit: 5 }),
      });
      found = JSON.stringify(search.json ?? "").includes("quokka");
      if (!found) await sleep(1000);
    }
    check("a search finds what was uploaded", found);
    const exported = await fetch(`${base}/v1/workspaces/${workspaceId}/export`, { headers: auth }).catch(() => null);
    const archive = exported?.ok ? Buffer.from(await exported.arrayBuffer()) : null;
    check("the workspace exports as an archive", Boolean(archive && archive.length > 100), `status ${exported?.status}`);
    if (archive && organization?.id) {
      const query = new URLSearchParams({ organization_id: organization.id, name: "Smoke Workspace (imported)" });
      const imported = await fetch(`${base}/v1/workspaces/import?${query}`, { method: "POST", headers: { ...auth, "content-type": "application/gzip", accept: "application/json" }, body: archive }).catch(() => null);
      const report = await imported?.json().catch(() => null);
      check("the archive imports as a new workspace", imported?.ok === true, report?.error?.message ?? `status ${imported?.status}`);
    }
  }
  if (process.env.REPOMEMO_SMOKE_BACKUP !== "1") {
    const started = await call("/v1/system/backups", { method: "POST", headers: jsonAuth, body: "{}" });
    check("a backup starts", started.status === 202, `status ${started.status}`);
    let finished = null;
    for (let attempt = 0; attempt < 120 && !finished; attempt += 1) {
      await sleep(1000);
      const state = await call("/v1/system/backups", { headers: auth });
      if (state.json && state.json.status.running === false && state.json.status.succeeded !== null) finished = state.json;
    }
    check("the backup finishes", finished?.status.succeeded === true, finished?.status.message ?? "timed out");
  }
}

const failed = results.filter((ok) => !ok).length;
console.log(`\n${results.length - failed} of ${results.length} checks passed.`);
process.exit(failed ? 1 : 0);
