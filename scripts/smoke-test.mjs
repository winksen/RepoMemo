// A check of a running RepoMemo server, for a clean-machine install (the
// production guide, V0.1.76 R6). Nothing in it changes data except an optional
// backup. Usage:
//
//   node scripts/smoke-test.mjs https://memo.example.com
//
// With REPOMEMO_SMOKE_EMAIL and REPOMEMO_SMOKE_PASSWORD (a system
// administrator) it also signs in, reads the overview, the metrics and the
// backups, and makes a backup when REPOMEMO_SMOKE_BACKUP=1.
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

const health = await call("/health");
check("GET /health answers 200", health.status === 200, `status ${health.status}`);
check("the service reports ok", health.json?.status === "ok");
const ready = await call("/health/ready");
check("GET /health/ready answers 200", ready.status === 200, ready.text.slice(0, 80));
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

const failed = results.filter((ok) => !ok).length;
console.log(`\n${results.length - failed} of ${results.length} checks passed.`);
process.exit(failed ? 1 : 0);
