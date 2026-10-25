// Checks the container files against each other and against the server
// (roadmap O7): the compose file builds stages the Dockerfile has, sets only
// variables the server reads, requires what .env.example explains, keeps the server and the LibreOffice
// container locked down (V0.2.148), points the
// proxy at the server's port and trusts the proxy's address. Nothing may move
// under it either: every base image is pinned by digest and every CI action by
// commit (roadmap E17). It reads files only: it starts nothing and needs no Docker.
//
//   node scripts/check-container.mjs

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const problems = [];
const read = (path) => {
  if (!existsSync(path)) {
    problems.push(`${path} is missing`);
    return "";
  }
  return readFileSync(path, "utf8");
};

const dockerfile = read("Dockerfile");
const workflows = readdirSync(".github/workflows")
  .filter((name) => /.ya?ml$/.test(name))
  .map((name) => [name, read(join(".github/workflows", name))]);
const compose = read("docker-compose.yml");
const caddyfile = read("docker/Caddyfile.edge");
const webCaddyfile = read("docker/Caddyfile.web");
const example = read(".env.example");
const ignore = read(".dockerignore");
const vite = read("apps/desktop/vite.config.ts");
const serverManifest = read("apps/server/Cargo.toml");

// No tabs in YAML, and files that Linux reads keep LF line endings in the repository.
if (/\t/.test(compose)) problems.push("docker-compose.yml contains a tab; YAML allows spaces only");
const attributes = read(".gitattributes");
for (const pattern of ["Dockerfile", "docker/Caddyfile.edge", "docker/Caddyfile.web", "docker/*.sh", "docker-compose.yml"]) {
  if (!attributes.split(/\r?\n/).some((line) => line.startsWith(pattern) && /eol=lf/.test(line))) {
    problems.push(`.gitattributes does not keep ${pattern} on LF line endings`);
  }
}

// Nothing moves: a base image names a digest, an action names a commit, and Dependabot keeps both current.
const earlierStages = new Set();
for (const match of dockerfile.matchAll(/^FROM\s+(\S+)(?:\s+AS\s+(\S+))?/gim)) {
  const [, image, stage] = match;
  if (image !== "scratch" && !earlierStages.has(image) && !/@sha256:[0-9a-f]{64}$/.test(image)) problems.push(`the Dockerfile's FROM ${image} names no digest (use image:tag@sha256:...)`);
  if (stage) earlierStages.add(stage);
}
for (const [name, text] of workflows) {
  for (const match of text.matchAll(/^\s*-?\s*uses:\s*(\S+)/gm)) {
    const [action, ref] = match[1].split("@");
    if (action.startsWith("./")) continue;
    if (!/^[0-9a-f]{40}$/.test(ref ?? "")) problems.push(`.github/workflows/${name} uses ${match[1]}, which is a tag or branch; pin it to a commit`);
  }
}
const dependabot = read(".github/dependabot.yml");
for (const ecosystem of ["cargo", "npm", "github-actions", "docker"]) {
  if (!new RegExp(String.raw`package-ecosystem:\s*${ecosystem}\b`).test(dependabot)) problems.push(`.github/dependabot.yml does not watch ${ecosystem}`);
}

// The server image's git must be one the remote-clone check accepts (S5-7): Debian 12 ships 2.39.5, which
// `is_patched_git` refuses, so remote repositories could not be cloned in the image.
const gitOfDebian = { bookworm: "2.39.5", trixie: "2.47.3" };
const baseRelease = /^FROM\s+debian:([a-z]+)(?:-slim)?(?:@\S+)?\s+AS\s+server\b/im.exec(dockerfile)?.[1];
const gitSource = read("crates/git/src/remote.rs");
const acceptedFrom = {};
for (const arm of /pub fn is_patched_git[\s\S]*?match minor \{([\s\S]*?)\n    \}/.exec(gitSource)?.[1].matchAll(/^\s*(\d+) => patch >= (\d+),/gm) ?? []) {
  acceptedFrom[Number(arm[1])] = Number(arm[2]);
}
let serverGit = "unknown";
if (!baseRelease || !(baseRelease in gitOfDebian)) {
  problems.push(`the server stage's base ${baseRelease ?? "(none)"} is not one this check knows the git release of (${Object.keys(gitOfDebian).join(", ")})`);
} else if (Object.keys(acceptedFrom).length === 0) {
  problems.push("crates/git/src/remote.rs: is_patched_git has no release table this check can read");
} else {
  serverGit = gitOfDebian[baseRelease];
  const [, minor, patch] = serverGit.split(".").map(Number);
  if (!(minor in acceptedFrom) || patch < acceptedFrom[minor]) {
    problems.push(`the server image's base (Debian ${baseRelease}) carries git ${serverGit}, which is_patched_git refuses; remote repositories could not be cloned`);
  }
}

// Stages: the compose targets exist, and the two images the guide names are there.
const stages = [...dockerfile.matchAll(/^FROM\s+\S+\s+AS\s+(\S+)/gim)].map((match) => match[1]);
for (const stage of ["web", "build", "server", "edge", "webapp"]) {
  if (!stages.includes(stage)) problems.push(`Dockerfile has no stage named ${stage}`);
}
for (const match of compose.matchAll(/^\s+target:\s*(\S+)/gm)) {
  if (!stages.includes(match[1])) problems.push(`docker-compose.yml builds the target ${match[1]}, which the Dockerfile does not have`);
}
for (const stage of ["server", "web".replace("web", "webapp"), "edge"]) {
  if (!new RegExp(`^\\s+target:\\s*${stage}\\s*$`, "m").test(compose)) problems.push(`docker-compose.yml does not build the ${stage} image`);
}

// Lock-down (S5-8, E1c): the server has a read-only root, no capabilities and no way to gain any; LibreOffice runs
// in a container of its own with no network that sees only the previews volume and its socket.
const service = (name) => {
  const match = new RegExp(`^  ${name}:\\n([\\s\\S]*?)(?=^  \\S|^\\S|(?![\\s\\S]))`, "m").exec(compose);
  return match ? match[1] : null;
};
const lockedDown = (name, block) => {
  if (!/^\s+read_only:\s*true\s*$/m.test(block)) problems.push(`docker-compose.yml: ${name} does not have a read-only root filesystem (read_only: true)`);
  if (!/^\s+tmpfs:\s*\n\s+-\s*\/tmp\s*$/m.test(block)) problems.push(`docker-compose.yml: ${name} has no tmpfs for /tmp`);
  if (!/^\s+cap_drop:\s*\n\s+-\s*ALL\s*$/m.test(block)) problems.push(`docker-compose.yml: ${name} does not drop every capability (cap_drop: [ALL])`);
  if (!/no-new-privileges:true/.test(block)) problems.push(`docker-compose.yml: ${name} lacks security_opt no-new-privileges:true`);
};
const serverBlock = service("server");
const converterBlock = service("converter");
if (!serverBlock) problems.push("docker-compose.yml has no server service");
else lockedDown("server", serverBlock);
if (converterBlock) {
  lockedDown("converter", converterBlock);
  if (!/^\s+network_mode:\s*none\s*$/m.test(converterBlock)) problems.push("docker-compose.yml: the converter has a network (network_mode: none)");
  if (!/^\s+profiles:\s*\n\s+-\s*previews\s*$/m.test(converterBlock)) problems.push("docker-compose.yml: the converter is not behind the previews profile");
  if (/repomemo-data/.test(converterBlock)) problems.push("docker-compose.yml: the converter sees the data volume; it may see only the previews volume and its socket");
  if (/ports:/.test(converterBlock)) problems.push("docker-compose.yml: the converter publishes a port");
  for (const volume of ["repomemo-previews", "soffice-socket"]) {
    if (!converterBlock.includes(volume) || !(serverBlock ?? "").includes(volume)) problems.push(`the server and the converter must share the ${volume} volume`);
  }
  if (!/target:\s*converter/.test(converterBlock) || !stages.includes("converter")) problems.push("the converter service needs the Dockerfile's converter stage");
  for (const file of ["docker/soffice-wrapper.sh", "docker/soffice-run.sh", "docker/soffice-listener.sh"]) {
    if (!existsSync(file)) problems.push(`${file} is missing`);
    else if (!dockerfile.includes(file)) problems.push(`the Dockerfile does not copy ${file}`);
  }
  if (!/REPOMEMO_SOFFICE_WRAPPER:\s*\/usr\/local\/bin\/repomemo-soffice-wrapper/.test(serverBlock ?? "")) problems.push("the server does not reach LibreOffice through the wrapper");
}

// CI really starts the compose file and builds the images for arm64 (V0.3.23): a job that runs `docker compose up`
// and waits for the server to be healthy, asks `/health/ready` through the edge, and a job on an arm64 runner that
// builds the stages the compose file builds for that platform.
const ci = workflows.find(([name]) => name === "ci.yml")?.[1] ?? "";
if (!/docker compose up\b[^\n]*--build[^\n]*--wait\b/.test(ci)) problems.push(".github/workflows/ci.yml has no job that builds the stack and waits for it (docker compose up --build --wait)");
if (!/curl[^\n]*https:\/\/localhost\/health\/ready/.test(ci)) problems.push(".github/workflows/ci.yml does not ask /health/ready through the edge");
if (!/docker compose down\b/.test(ci)) problems.push(".github/workflows/ci.yml does not take the stack down again");
if (!/runs-on:\s*ubuntu-24\.04-arm\b/.test(ci)) problems.push(".github/workflows/ci.yml has no job on an arm64 runner");
for (const stage of ["server", "webapp", "edge"]) {
  if (!new RegExp(`docker build[^\\n]*--platform linux/arm64[^\\n]*--target ${stage}\\b`).test(ci)) problems.push(`.github/workflows/ci.yml does not build the ${stage} image for arm64`);
}

// The binary the Dockerfile copies is the one the server package builds.
const packageName = /^name\s*=\s*"([^"]+)"/m.exec(serverManifest)?.[1];
if (!packageName || !dockerfile.includes(`-p ${packageName}`) || !dockerfile.includes(`target/release/${packageName}`)) {
  problems.push(`the Dockerfile does not build and copy the server package (${packageName ?? "unknown"})`);
}
// The server image starts the entrypoint (V0.4.10), which fixes the ownership of a volume that belongs to
// someone else and drops to the unprivileged user before the server runs; it never runs the server as root.
const entrypoint = read("docker/entrypoint.sh");
if (!/ENTRYPOINT \["repomemo-entrypoint"\]/.test(dockerfile)) problems.push("the server image does not start repomemo-entrypoint");
if (!dockerfile.includes("docker/entrypoint.sh")) problems.push("the Dockerfile does not copy docker/entrypoint.sh");
if (!/exec setpriv --reuid="\$uid" --regid="\$uid" --clear-groups repomemo-server/.test(entrypoint)) problems.push("docker/entrypoint.sh does not drop to the unprivileged user before it runs the server");
if (!/uid=10001\b/.test(entrypoint) || !/useradd[^\n]*--uid 10001/.test(dockerfile)) problems.push("the entrypoint and the Dockerfile do not name the same unprivileged user (10001)");
if (!/chown -R/.test(entrypoint)) problems.push("docker/entrypoint.sh does not give a volume to the unprivileged user");
{
  const serverStage = dockerfile.slice(dockerfile.indexOf("AS server"), dockerfile.indexOf("AS converter"));
  if (/^USER\s+(root|0)/m.test(serverStage)) problems.push("the server stage sets USER root; the entrypoint drops the privilege itself");
}
if (!/^\s+cap_add:\s*\n(\s+-\s*(CHOWN|DAC_OVERRIDE|FOWNER|SETUID|SETGID)\s*\n)+/m.test(serverBlock ?? "")) problems.push("docker-compose.yml: the server needs CHOWN, DAC_OVERRIDE, FOWNER, SETUID and SETGID for its entrypoint, and no more");
if (/^\s+-\s*(SYS_ADMIN|NET_ADMIN|NET_RAW|SYS_PTRACE|ALL)\s*$/m.test((serverBlock ?? "").split("cap_add:")[1] ?? "")) problems.push("docker-compose.yml: the server adds a capability beyond its entrypoint's");
if (/^\s*(ENV|ARG)\s+\S*(SECRET|PASSWORD|TOKEN|JWT)\S*=/im.test(dockerfile)) problems.push("the Dockerfile bakes a secret into an image");

// A change of code does not build the dependencies again (V0.4.7): they are cooked from a recipe in a layer of
// their own, before the code is copied in, and no cache mount over the build directory hides that layer.
const cooked = dockerfile.indexOf("cargo chef cook");
const copiedCode = dockerfile.indexOf("COPY . .", dockerfile.indexOf("AS build"));
if (cooked < 0 || copiedCode < 0 || cooked > copiedCode) problems.push("the server build does not cook its dependencies before it copies the code (cargo chef cook, then COPY . .)");
if (/--mount=type=cache,target=\/build\/target/.test(dockerfile)) problems.push("a cache mount over /build/target hides the layer of built dependencies");
if (!/cargo install cargo-chef --locked --version \d+\.\d+\.\d+/.test(dockerfile)) problems.push("cargo-chef is not installed at a pinned version");
if (!workflows.some(([, text]) => text.includes("server-image-cache:"))) problems.push("CI has no server-image-cache job that measures the rebuild");

// A smaller server image (V0.4.8): the binary is stripped, and the health check is the server's own --health,
// so the image carries no curl. socat stays for the converter's socket (repomemo-soffice-wrapper).
if (!/strip --strip-unneeded/.test(dockerfile)) problems.push("the server binary is not stripped");
if (!/HEALTHCHECK[^\n]*\\\n\s*CMD \["repomemo-server", "--health"\]/.test(dockerfile)) problems.push("the server image's health check is not repomemo-server --health");
{
  const serverStage = dockerfile.slice(dockerfile.indexOf("AS server"), dockerfile.indexOf("AS converter"));
  if (/\bcurl\b/.test(serverStage.replace(/#[^\n]*/g, ""))) problems.push("the server image installs or runs curl; the health check is repomemo-server --health");
}

// Every image says what it is (V0.4.9): the same labels on the server, the converter and the edge, from the
// build arguments the compose file gives each of them.
for (const stage of ["server", "converter", "edge", "webapp"]) {
  const start = dockerfile.search(new RegExp(String.raw`^FROM\s.*\sAS\s${stage}\s*$`, "m"));
  const next = dockerfile.slice(start + 1).search(/^FROM\s/m);
  const body = start < 0 ? "" : dockerfile.slice(start, next < 0 ? undefined : start + 1 + next);
  for (const key of ["title", "description", "source", "licenses", "version", "revision", "created"]) {
    if (!body.includes(`org.opencontainers.image.${key}=`)) problems.push(`the ${stage} image has no org.opencontainers.image.${key} label`);
  }
  for (const name of ["REPOMEMO_BUILD_COMMIT", "REPOMEMO_BUILD_DATE", "REPOMEMO_IMAGE_TAG"]) {
    if (!new RegExp(String.raw`^ARG\s+${name}\b`, "m").test(body)) problems.push(`the ${stage} stage does not take the build argument ${name} its labels use`);
  }
  const service = new RegExp(String.raw`target:\s*${stage}\s*\n\s*args:([\s\S]*?)(?=\n    \S)`).exec(compose)?.[1] ?? "";
  for (const name of ["REPOMEMO_BUILD_COMMIT", "REPOMEMO_BUILD_DATE", "REPOMEMO_IMAGE_TAG"]) {
    if (!service.includes(name)) problems.push(`the compose file does not pass ${name} to the ${stage} image`);
  }
}

// The server image is told what it is (V0.4.3): the Dockerfile takes the three build arguments, and the compose file gives them.
for (const name of ["REPOMEMO_BUILD_COMMIT", "REPOMEMO_BUILD_DATE", "REPOMEMO_IMAGE_TAG"]) {
  if (!new RegExp(String.raw`^ARG\s+${name}\b`, "m").test(dockerfile)) problems.push(`the Dockerfile has no ARG ${name}, so /health cannot name the build`);
  if (!compose.includes(`${name}:`)) problems.push(`the compose file gives no build argument ${name}`);
}

// Every REPOMEMO_ variable the compose file sets is one the server (or the proxy) reads.
const serverSource = [];
const walk = (folder) => {
  for (const entry of readdirSync(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) walk(path);
    else if (entry.name.endsWith(".rs")) serverSource.push(readFileSync(path, "utf8"));
  }
};
walk("apps/server/src");
const sourceText = serverSource.join("\n");
const composeOnly = new Set(["REPOMEMO_HOST", "REPOMEMO_VERSION", "REPOMEMO_CONVERTER_IMAGE", "REPOMEMO_WEB_API_URL", "REPOMEMO_PRODUCT_NAME", "REPOMEMO_API_UPSTREAM", "REPOMEMO_WEB_PORT", "REPOMEMO_TLS_DIR"]);
const used = new Set([...compose.matchAll(/\b(REPOMEMO_[A-Z0-9_]+)\b/g)].map((match) => match[1]));
for (const name of used) {
  if (!composeOnly.has(name) && !sourceText.includes(`"${name}"`)) problems.push(`docker-compose.yml sets ${name}, which the server does not read`);
}
for (const name of ["REPOMEMO_HOST", "REPOMEMO_VERSION"]) {
  if (!example.includes(`${name}=`)) problems.push(`.env.example does not explain ${name}`);
}

// What the compose file requires, .env.example explains.
for (const match of compose.matchAll(/\$\{(REPOMEMO_[A-Z0-9_]+):\?/g)) {
  if (!new RegExp(`^${match[1]}=`, "m").test(example)) problems.push(`${match[1]} is required by docker-compose.yml but not in .env.example`);
}
for (const match of example.matchAll(/^(REPOMEMO_[A-Z0-9_]+)=/gm)) {
  if (!used.has(match[1])) problems.push(`.env.example has ${match[1]}, which docker-compose.yml does not use`);
}

// The proxy reaches the server where it listens, and the server trusts the proxy's address.
const listens = /REPOMEMO_SERVER_ADDR:\s*0\.0\.0\.0:(\d+)/.exec(compose)?.[1];
if (!listens) problems.push("docker-compose.yml does not make the server listen on all interfaces");
else if (!new RegExp(`REPOMEMO_API_UPSTREAM:\\s*server:${listens}\\b`).test(compose)) problems.push(`docker-compose.yml does not send the web image's API rule to server:${listens}`);
if (!/reverse_proxy \{\$REPOMEMO_EDGE_UPSTREAM:web:8080\}/.test(caddyfile)) problems.push("docker/Caddyfile.edge does not send everything to the web image on 8080");
// The health check (repomemo-server --health, V0.4.8) asks the address in REPOMEMO_SERVER_ADDR, which the image and the compose file set alike.
if (listens && !new RegExp(`REPOMEMO_SERVER_ADDR=0\\.0\\.0\\.0:${listens}\\b`).test(dockerfile)) problems.push("the server image's REPOMEMO_SERVER_ADDR, which its health check asks, is not the port the server listens on");
// Two hops carry the client's address (V0.4.18): the edge, then the web image; the server trusts both.
const proxyAddress = /^  edge:\n[\s\S]*?ipv4_address:\s*(\S+)/m.exec(compose)?.[1];
const webAddress = /^  web:\n[\s\S]*?ipv4_address:\s*(\S+)/m.exec(compose)?.[1];
const trusted = /REPOMEMO_TRUSTED_PROXIES:\s*(\S+)/.exec(compose)?.[1];
if (!proxyAddress || !webAddress || trusted !== `${proxyAddress},${webAddress}`) problems.push(`the server must trust the edge and the web image (edge ${proxyAddress ?? "none"}, web ${webAddress ?? "none"}, trusted ${trusted ?? "none"})`);
if (!/trusted_proxies static/.test(webCaddyfile)) problems.push("docker/Caddyfile.web does not keep the client's address from the edge (trusted_proxies)");
const subnet = /subnet:\s*(\d+\.\d+\.\d+)\.\d+\/\d+/.exec(compose)?.[1];
for (const address of [...compose.matchAll(/ipv4_address:\s*(\S+)/g)].map((match) => match[1])) {
  if (!subnet || !address.startsWith(`${subnet}.`)) problems.push(`${address} is outside the compose network`);
}
if (!/REPOMEMO_TRUST_PROXY:\s*"?true"?/.test(compose)) problems.push("docker-compose.yml does not turn REPOMEMO_TRUST_PROXY on");
if (!/REPOMEMO_PUBLIC_URL:\s*https:\/\//.test(compose)) problems.push("docker-compose.yml does not give the server an https:// public address");
if (!/repomemo-data:\/var\/lib\/repomemo/.test(compose) || !/^volumes:\s*\n\s+repomemo-data:/m.test(compose)) problems.push("the data folder is not in a named volume");
if (!/WORKDIR \/var\/lib\/repomemo/.test(dockerfile)) problems.push("the server's working directory is not the volume");

// The web app is built for the page's own host, and the content security policy reaches the proxy.
if (!/VITE_REPOMEMO_API_URL=""/.test(dockerfile)) problems.push("the web app is not built for the page's own address");
if (/VITE_REPOMEMO_API_URL\s*\|\|/.test(vite)) problems.push("vite.config.ts treats an empty API address as unset, which would put 127.0.0.1 into the content security policy");
if (!vite.includes("content-security-policy.txt") || !dockerfile.includes("content-security-policy.txt")) problems.push("the content security policy file the build writes is not given to the proxy");
if (!webCaddyfile.includes("import /etc/caddy/csp.caddy") || !dockerfile.includes("/etc/caddy/csp.caddy")) problems.push("the Caddyfile and the Dockerfile disagree about the content security policy snippet");
if (!caddyfile.includes("flush_interval -1")) problems.push("the edge buffers the live event streams");
// The web app is told its API address and name when it starts (V0.4.14): the proxy serves /config.json from the environment the compose file gives it.
for (const name of ["REPOMEMO_WEB_API_URL", "REPOMEMO_PRODUCT_NAME"]) {
  if (!webCaddyfile.includes(`{$${name}:`)) problems.push(`docker/Caddyfile.web does not read ${name} for /config.json`);
  if (!compose.includes(`${name}:`)) problems.push(`docker-compose.yml does not give the web image ${name}`);
}
if (!/handle \/config\.json/.test(webCaddyfile)) problems.push("docker/Caddyfile.web does not serve /config.json");
if (!/vite\.config\.ts/.test("vite.config.ts") || !vite.includes("config.json")) problems.push("the web build does not write a default config.json");
if (!/\{\$REPOMEMO_HOST/.test(caddyfile)) problems.push("docker/Caddyfile.edge does not use REPOMEMO_HOST");
// Sample front doors (V0.4.19): each names the server's trusted-proxy setting, forwards the client's address, turns
// buffering off for the live event streams and sends HSTS.
{
  const nginx = read("deploy/proxies/nginx.conf");
  const traefik = read("deploy/proxies/traefik.yml");
  for (const [name, text] of [["nginx.conf", nginx], ["traefik.yml", traefik]]) {
    if (!text.includes("REPOMEMO_TRUST_PROXY=true") || !text.includes("REPOMEMO_TRUSTED_PROXIES")) problems.push(`deploy/proxies/${name} does not name the server's trusted-proxy settings`);
    if (!text.includes("REPOMEMO_PUBLIC_URL") || !text.includes("REPOMEMO_ALLOWED_ORIGIN")) problems.push(`deploy/proxies/${name} does not name the server's public address and origin`);
  }
  if (!/proxy_buffering off;/.test(nginx) || !/proxy_set_header X-Forwarded-For \$remote_addr;/.test(nginx)) problems.push("deploy/proxies/nginx.conf does not turn buffering off for the event streams or set X-Forwarded-For to the client");
  if (!/location ~ \^\/v1\/\(workspaces\/\[\^\/\]\+\/events\|system\/events\)\$/.test(nginx)) problems.push("deploy/proxies/nginx.conf has no location for the live event streams");
  if (!/Strict-Transport-Security/.test(nginx) || !/stsSeconds/.test(traefik)) problems.push("a sample front door does not send HSTS");
  if (!/flushInterval:\s*"?1ms"?/.test(traefik)) problems.push("deploy/proxies/traefik.yml does not flush the live event streams at once");
  if (/forwardedHeaders:\s*\n\s+insecure:\s*true/.test(traefik)) problems.push("deploy/proxies/traefik.yml trusts forwarded headers from anyone");
  for (const route of ["/v1/workspaces/{workspace_id}/events", "/v1/system/events"]) {
    if (!readFileSync("apps/server/src/lib.rs", "utf8").includes(route)) problems.push(`the server has no ${route}, which the sample front doors name`);
  }
}

// TLS is the edge's alone (V0.4.18): optional behind a profile, three ways to a certificate, never in the web image.
{
  const edgeBlock = /^  edge:\n([\s\S]*?)(?=^  \S|^\S|(?![\s\S]))/m.exec(compose)?.[1] ?? "";
  if (!/^\s+profiles:\s*\n\s+-\s*edge\s*$/m.test(edgeBlock)) problems.push("docker-compose.yml: the edge is not behind the edge profile");
  if (!/\/certs:ro/.test(edgeBlock)) problems.push("docker-compose.yml: the edge does not mount the operator's certificates read-only at /certs");
  if (!/^\s+cap_add:\s*\n\s+-\s*NET_BIND_SERVICE\s*$/m.test(edgeBlock) || !/^\s+cap_drop:\s*\n\s+-\s*ALL\s*$/m.test(edgeBlock)) problems.push("docker-compose.yml: the edge should drop every capability but NET_BIND_SERVICE");
  const entrypoint = read("docker/edge-entrypoint.sh");
  if (!entrypoint.includes("/certs/tls.crt") || !entrypoint.includes("/certs/tls.key")) problems.push("docker/edge-entrypoint.sh does not use the certificates mounted at /certs");
  if (!caddyfile.includes("import /tmp/tls.caddy") || !/ENTRYPOINT \["repomemo-edge-entrypoint"\]/.test(dockerfile)) problems.push("the edge does not take its certificate rule from its entrypoint");
  if (/tls internal|auto_https/.test(webCaddyfile.replace(/auto_https off/, ""))) problems.push("docker/Caddyfile.web carries TLS settings; TLS belongs to the edge");
  if (!/COMPOSE_PROFILES=edge/.test(example)) problems.push(".env.example does not ask for the edge profile");
  if (!/echo "COMPOSE_PROFILES=edge"/.test(ci) || !/^  compose-no-edge:/m.test(ci) || !/^  edge-certificates:/m.test(ci)) problems.push(".github/workflows/ci.yml does not try the stack with the edge, without it and with supplied certificates");
}

// Nothing private goes into the build context.
for (const pattern of [".git", ".env", "workspace-data", "target"]) {
  if (!ignore.split(/\r?\n/).includes(pattern)) problems.push(`.dockerignore does not leave out ${pattern}`);
}

// The web client has an image of its own (V0.4.15): plain HTTP on 8080, no API needed, hashed files cached and the page
// never, a health route, an unprivileged user and a CI job that runs it alone.
{
  const webCaddy = read("docker/Caddyfile.web");
  if (!stages.includes("webapp")) problems.push("the Dockerfile has no webapp stage (the web image)");
  for (const [needle, what] of [
    [":8080", "listens on 8080"], ["auto_https off", "leaves TLS to a front door"], ["/healthz", "has a health route"],
    ["/config.json", "serves /config.json"], ["immutable", "caches hashed files"], ["no-cache", "revalidates the page"],
    ["try_files {path} /index.html", "falls back to the page for client routes"], ["csp.caddy", "sends the content security policy"],
    ["nosniff", "sets the security headers"],
  ]) {
    if (!webCaddy.includes(needle)) problems.push(`docker/Caddyfile.web does not serve what the web image promises: it never ${what}`);
  }
  const webStage = dockerfile.slice(dockerfile.search(/^FROM\s.*\sAS\s+webapp\s*$/m));
  if (!/^USER\s+10001\b/m.test(webStage)) problems.push("the web image does not run as an unprivileged user");
  if (!/HEALTHCHECK[\s\S]*\/healthz/.test(webStage)) problems.push("the web image has no health check on /healthz");
  if (!/docker build[^\n]*--target webapp\b/.test(ci) || !/--publish 127\.0\.0\.1:8080:8080[\s\S]*--read-only[\s\S]*--cap-drop ALL/.test(ci)) problems.push(".github/workflows/ci.yml has no job that runs the web image alone, read-only and without capabilities");
}

// Cross-origin mode (V0.4.17): the web image puts the API's origin into the policy it sends when it starts, the page
// carries no policy of its own that could narrow it, and CI tries two origins and a third.
{
  const script = read("docker/web-entrypoint.sh");
  const webStage = dockerfile.slice(dockerfile.search(/^FROM\s.*\sAS\s+webapp\s*$/m));
  if (!script.includes("REPOMEMO_WEB_API_ORIGIN") || !script.includes("REPOMEMO_WEB_API_URL")) problems.push("docker/web-entrypoint.sh does not work the API's origin out of REPOMEMO_WEB_API_URL");
  if (!webStage.includes("{\\$REPOMEMO_WEB_API_ORIGIN:}")) problems.push("the web image's policy does not take the API's origin when it starts");
  if (!/sed -i '[^']*Content-Security-Policy[^']*d' \/srv\/web\/index\.html/.test(webStage)) problems.push("the web image leaves the build's own policy in the page, which would narrow the one it sends");
  if (!/\[!\]A-Za-z0-9/.test(script)) problems.push("docker/web-entrypoint.sh checks its values with a bracket expression that does not close on ]");
  if (!/node scripts\/cross-origin-test\.mjs/.test(ci) || !/REPOMEMO_COOKIE_SAMESITE=lax/.test(ci)) problems.push(".github/workflows/ci.yml has no job that tries the API and the web app on two origins");
}

// Same-origin mode (V0.4.16): the web image forwards /v1 and /health to REPOMEMO_API_UPSTREAM with the live event
// streams unbuffered, from a snippet its entrypoint writes; CI runs it together with the API.
{
  const webCaddy = read("docker/Caddyfile.web");
  const script = read("docker/web-entrypoint.sh");
  if (!webCaddy.includes("import /tmp/api.caddy")) problems.push("docker/Caddyfile.web does not take its API rule from /tmp/api.caddy");
  if (!/flush_interval -1/.test(script) || !/reverse_proxy \$target/.test(script)) problems.push("docker/web-entrypoint.sh does not forward to the upstream with the live event streams unbuffered");
  if (!script.includes("REPOMEMO_API_UPSTREAM")) problems.push("docker/web-entrypoint.sh does not read REPOMEMO_API_UPSTREAM");
  if (!/ENTRYPOINT \["repomemo-web-entrypoint"\]/.test(dockerfile) || !dockerfile.includes("docker/web-entrypoint.sh")) problems.push("the web image does not start its entrypoint");
  if (!/REPOMEMO_API_UPSTREAM=api:3020/.test(ci) || !/http:\/\/127\.0\.0\.1:8080\/health\/ready/.test(ci)) problems.push(".github/workflows/ci.yml has no job that asks the API through the web image");
}

// The images are scanned in CI (V0.4.13): a scanner container pinned by digest reads the server, the converter and
// the proxy, a high or critical finding fails the job, and the reports are kept.
{
  const job = /^  image-scan:\n([\s\S]*?)(?=^  \S|(?![\s\S]))/m.exec(ci)?.[1] ?? "";
  if (!job) problems.push(".github/workflows/ci.yml has no image-scan job");
  else {
    if (!/TRIVY_IMAGE:\s*\S+@sha256:[0-9a-f]{64}\b/.test(job)) problems.push("the image-scan job's scanner is not pinned by digest");
    if (!/--severity HIGH,CRITICAL/.test(job) || !/--exit-code 1/.test(job)) problems.push("the image-scan job does not fail on a high or critical finding");
    for (const [name, target] of [["server", "server"], ["converter", "converter"], ["edge", "edge"], ["web", "webapp"]]) {
      if (!new RegExp(String.raw`--target ${target}\b[^\n]*--tag repomemo-${name}:scan`).test(job)) problems.push(`the image-scan job does not build the ${name} image`);
    }
    if (!/upload-artifact@/.test(job)) problems.push("the image-scan job does not keep its reports");
  }
  if (!existsSync(".trivyignore")) problems.push(".trivyignore is missing (it may be empty)");
}

// The converter has an image of its own (V0.4.12): its compose service can start a tag given by
// REPOMEMO_CONVERTER_IMAGE, and CI builds the converter stage alone and starts it as the previews profile does.
if (!/image:\s*\$\{REPOMEMO_CONVERTER_IMAGE:-repomemo-converter:/.test(converterBlock ?? "")) problems.push("docker-compose.yml: the converter cannot start an image given by REPOMEMO_CONVERTER_IMAGE");
if (!/docker build[^\n]*--target converter\b[^\n]*--tag repomemo-converter/.test(ci) || !/--network none[^\n]*--read-only|--read-only[^\n]*--network none/.test(ci.replace(/\\\n\s*/g, " "))) problems.push(".github/workflows/ci.yml has no job that builds the converter image alone and runs it with no network and a read-only root");

// The API on its own (V0.4.11): deploy/api.yml is the server and its volume, one published port on the loopback,
// no web client and no proxy; it is locked down like the server of the full stack, and CI starts it.
{
  const api = read("deploy/api.yml");
  const declared = api.slice(api.indexOf("\nservices:"), api.indexOf("\nvolumes:"));
  const names = [...declared.matchAll(/^  ([a-z][\w-]*):\s*$/gm)].map((match) => match[1]);
  if (names.length !== 1 || names[0] !== "server") problems.push(`deploy/api.yml should hold the server alone, not ${names.join(", ") || "nothing"}`);
  if (!/target:\s*server\b/.test(api)) problems.push("deploy/api.yml does not build the server image");
  if (!/^\s+read_only:\s*true\s*$/m.test(api) || !/^\s+cap_drop:\s*\n\s+-\s*ALL\s*$/m.test(api) || !/no-new-privileges:true/.test(api)) problems.push("deploy/api.yml does not lock the server down like docker-compose.yml");
  if ((api.match(/^\s+ports:\s*$/gm) ?? []).length !== 1 || (api.match(/^\s+-\s*"127\.0\.0\.1:/gm) ?? []).length !== 1) problems.push("deploy/api.yml should publish one port, on the loopback");
  if (/\b(proxy|web|caddy)\b:/.test(declared)) problems.push("deploy/api.yml carries a web client or a proxy");
  for (const match of api.matchAll(/^\s+(REPOMEMO_[A-Z0-9_]+):/gm)) {
    if (!sourceText.includes(`"${match[1]}"`)) problems.push(`deploy/api.yml sets ${match[1]}, which the server does not read`);
  }
  if (!/REPOMEMO_JWT_SECRET:\s*\$\{REPOMEMO_JWT_SECRET:\?/.test(api) || !/REPOMEMO_SECRET_KEY:\s*\$\{REPOMEMO_SECRET_KEY:\?/.test(api)) problems.push("deploy/api.yml does not require the two secrets");
  if (!/docker compose -f deploy\/api\.yml[^\n]*up\b[^\n]*--wait\b/.test(ci) || !/curl[^\n]*127\.0\.0\.1:3020\/health\/ready/.test(ci)) problems.push(".github/workflows/ci.yml does not start deploy/api.yml and ask /health/ready on its port");
}

if (problems.length) {
  console.error(`The container files have ${problems.length} problem${problems.length === 1 ? "" : "s"}:`);
  for (const problem of problems) console.error(` - ${problem}`);
  process.exit(1);
}
console.log(`Container files agree (${stages.length} stages, ${used.size} variables, proxy ${proxyAddress} to server:${listens}, git ${serverGit} on Debian ${baseRelease}).`);
