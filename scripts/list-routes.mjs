// Lists every route of the server with its handler, authentication extractor,
// authorization calls and request and response types.
// usage: node routes.mjs [--markdown]
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const root = "apps/server/src";
function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : path.endsWith(".rs") ? [path] : [];
  });
}
const sources = files(root).filter((path) => !path.includes("/tests/") && !path.includes("\\tests\\"));
// Line ends are normalised so a Windows checkout reads like a Linux one.
const texts = Object.fromEntries(sources.map((path) => [path, readFileSync(path, "utf8").split("\r\n").join("\n")]));

// Handlers: name -> { file, signature, body }
const handlers = new Map();
for (const [path, text] of Object.entries(texts)) {
  const pattern = /(?:pub(?:\(crate\))? )?async fn (\w+)\s*(?:<[^>]*>)?\(([\s\S]*?)\)\s*->\s*([\s\S]*?)\s*\{\n/g;
  let match;
  while ((match = pattern.exec(text))) {
    const start = match.index + match[0].length;
    let depth = 1;
    let index = start;
    while (depth > 0 && index < text.length) {
      const char = text[index++];
      if (char === "{") depth++;
      else if (char === "}") depth--;
    }
    const module = path.split(/[\\/]/).pop().replace(".rs", "");
    handlers.set(`${module}::${match[1]}`, { file: path, params: match[2], returns: match[3], body: text.slice(start, index) });
    if (!handlers.has(match[1])) handlers.set(match[1], handlers.get(`${module}::${match[1]}`));
  }
}

const lib = texts[join(root, "lib.rs")];
const routes = [];
// A handler may carry a layer of its own (`post(login).layer(DefaultBodyLimit::max(LIMIT))`, V0.2.52).
const routePattern = /\.route\(\s*"([^"]+)",\s*((?:\s*[a-z]+\([\w:]+\)(?:\.layer\(\w+::\w+\([\w:]+\)\))?\s*\.?)+)\s*,?\s*\)/g;
let route;
while ((route = routePattern.exec(lib))) {
  for (const call of route[2].replace(/\.layer\(\w+::\w+\([\w:]+\)\)/g, "").matchAll(/([a-z]+)\(([\w:]+)\)/g)) {
    routes.push({ method: call[1].toUpperCase(), path: route[1], handler: call[2] });
  }
}

const guards = [
  "require_workspace_read", "require_workspace_write", "require_workspace_admin", "require_workspace_owner",
  "require_organization_read", "require_organization_admin", "require_organization_owner",
  "require_system_admin", "require_full_system_admin", "require_system_change",
  "authorize_ai_use", "consume_ai_quota", "effective_workspace_role", "require_session",
];
const rows = routes.map((entry) => {
  const key = entry.handler.includes("::") ? entry.handler : entry.handler;
  const found = handlers.get(key) ?? handlers.get(key.split("::").pop());
  if (!found) return { ...entry, authn: "?", guards: [], request: "", response: "", note: "handler not found" };
  const authn = /SessionSubject/.test(found.params)
    ? "session"
    : /ManagingSubject/.test(found.params)
      ? "managing"
      : /ReadingSubject/.test(found.params)
      ? "reading"
      : /AuthenticatedSubject/.test(found.params)
        ? "auth"
        : /ClientIp/.test(found.params) || !/Subject/.test(found.params)
          ? "public"
          : "?";
  const used = guards.filter((guard) => found.body.includes(guard));
  const request = [...found.params.matchAll(/Json<([^>]+(?:<[^>]*>)?)>/g)].map((m) => m[1]).join(", ");
  const response = (found.returns.match(/Json<([^;{]+?)>(?:,|\))?/) ?? [])[1] ?? (found.returns.includes("StatusCode") ? "status" : "");
  return { ...entry, authn, guards: used, request, response, note: "" };
});

if (process.argv.includes("--markdown")) {
  for (const row of rows) {
    console.log(`| ${row.method} | \`${row.path}\` | ${row.authn} | ${row.guards.join(", ") || "—"} | ${row.request || "—"} | ${row.response || "—"} |`);
  }
} else {
  for (const row of rows) {
    const flag = (row.authn !== "public" && row.guards.length === 0) || row.authn === "?" ? "  <-- check" : "";
    console.log(`${row.method.padEnd(6)} ${row.path.padEnd(64)} ${row.handler.padEnd(44)} ${row.authn.padEnd(8)} ${row.guards.join(",")}${flag}`);
  }
  console.log(`\n${rows.length} routes`);
}
