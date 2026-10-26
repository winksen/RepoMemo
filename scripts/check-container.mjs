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
import { pinProblems } from "./pins.mjs";

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
const composeBuild = read("docker-compose.build.yml");
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
// The rules are in scripts/pins.mjs (V0.4.42), with tests: every workflow, the Dockerfile, the compose files, the examples and the
// scripts that run an image are read, so a new file is covered without being listed.
const pinnedFiles = [
  ["docker-compose.yml", compose],
  ["docker-compose.build.yml", composeBuild],
  ...["deploy/api.yml", ...readdirSync("deploy").filter((name) => name.endsWith(".sh")).map((name) => `deploy/${name}`), ...readdirSync("deploy/direct/service").map((name) => `deploy/direct/service/${name}`), "deploy/init.ps1", "deploy/monitoring/prometheus.yml", ".env.example"].map((path) => [path, read(path)]),
];
problems.push(...pinProblems({ workflows, dockerfile, others: pinnedFiles }));
const dependabot = read(".github/dependabot.yml");
for (const ecosystem of ["cargo", "npm", "github-actions", "docker", "docker-compose"]) {
  if (!new RegExp(String.raw`package-ecosystem:\s*${ecosystem}\b`).test(dependabot)) problems.push(`.github/dependabot.yml does not watch ${ecosystem}`);
}
// Dependabot reads workflows from /.github/workflows and the Dockerfile from the directory it is told: both must be the root.
for (const ecosystem of ["github-actions", "docker", "docker-compose"]) {
  if (!new RegExp(String.raw`package-ecosystem:\s*${ecosystem}\b[^\n]*\n\s*directory:\s*/\s*\n`).test(dependabot)) problems.push(`.github/dependabot.yml: ${ecosystem} is not read from the repository root`);
}
if (!/node --test scripts\/pins\.test\.mjs/.test(workflows.find(([name]) => name === "ci.yml")?.[1] ?? "")) problems.push("CI does not run the pinning rules' tests (scripts/pins.test.mjs)");
const workflowFiles = readdirSync(".github/workflows").filter((name) => /\.ya?ml$/.test(name));
if (workflowFiles.length < 3) problems.push("the workflows folder lost a workflow (ci, server-archives and release-images are expected)");

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
for (const match of composeBuild.matchAll(/^\s+target:\s*(\S+)/gm)) {
  if (!stages.includes(match[1])) problems.push(`docker-compose.build.yml builds the target ${match[1]}, which the Dockerfile does not have`);
}
for (const stage of ["server", "converter", "webapp", "edge"]) {
  if (!new RegExp(`^\\s+target:\\s*${stage}\\s*$`, "m").test(composeBuild)) problems.push(`docker-compose.build.yml does not build the ${stage} image`);
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
  if (!/target:\s*converter/.test(composeBuild) || !stages.includes("converter")) problems.push("the converter service needs the Dockerfile's converter stage");
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
  // The build file gives each image the arguments through one anchor (x-build-args), which holds all three.
  if (!new RegExp(String.raw`target:\s*${stage}\s*\n\s*args:\s*\*build-args`).test(composeBuild)) problems.push(`the build file does not pass the build arguments to the ${stage} image`);
}
const buildArgs = /x-build-args:\s*&build-args\n((?:  .*\n)+)/.exec(composeBuild)?.[1] ?? "";
for (const name of ["REPOMEMO_BUILD_COMMIT", "REPOMEMO_BUILD_DATE", "REPOMEMO_IMAGE_TAG"]) {
  if (!buildArgs.includes(name)) problems.push(`the build file does not pass ${name} to the images`);
}

// The server image is told what it is (V0.4.3): the Dockerfile takes the three build arguments, and the compose file gives them.
for (const name of ["REPOMEMO_BUILD_COMMIT", "REPOMEMO_BUILD_DATE", "REPOMEMO_IMAGE_TAG"]) {
  if (!new RegExp(String.raw`^ARG\s+${name}\b`, "m").test(dockerfile)) problems.push(`the Dockerfile has no ARG ${name}, so /health cannot name the build`);
  if (!composeBuild.includes(`${name}:`)) problems.push(`the build file gives no build argument ${name}`);
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
const composeOnly = new Set(["REPOMEMO_HOST", "REPOMEMO_VERSION", "REPOMEMO_CONVERTER_IMAGE", "REPOMEMO_REGISTRY", "REPOMEMO_WEB_API_URL", "REPOMEMO_PRODUCT_NAME", "REPOMEMO_API_UPSTREAM", "REPOMEMO_WEB_PORT", "REPOMEMO_TLS_DIR", "REPOMEMO_METRICS_TOKEN_FILE"]);
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
// The server as a release archive (V0.4.31): one packager, the same layout on every system, tried in CI by unpacking in a
// clean folder and starting the server on the web app inside it.
{
  const packager = read("scripts/package-server.mjs");
  for (const needle of ["bin/", "web/", "config/repomemo.env.example", "service/", "README.txt", "SHA256SUMS", "VERSION"]) {
    if (!packager.includes(needle)) problems.push(`scripts/package-server.mjs does not put ${needle} in the archive`);
  }
  if (!/repomemo-server\.exe/.test(packager) || !/tar\.gz/.test(packager) || !/zip/.test(packager)) problems.push("scripts/package-server.mjs does not make a tar.gz for Linux and a zip with repomemo-server.exe for Windows");
  const env = read("deploy/direct/repomemo.env.example");
  for (const name of ["REPOMEMO_SERVER_ADDR", "REPOMEMO_PUBLIC_URL", "REPOMEMO_WEB_DIR", "REPOMEMO_JWT_SECRET", "REPOMEMO_SECRET_KEY", "REPOMEMO_SERVER_DATA_DIR"]) {
    if (!new RegExp(`^${name}=`, "m").test(env)) problems.push(`deploy/direct/repomemo.env.example does not set ${name}`);
    if (!sourceText.includes(`"${name}"`)) problems.push(`deploy/direct/repomemo.env.example sets ${name}, which the server does not read`);
  }
  if (!/^  server-archive:/m.test(ci) || !/package-server\.mjs --binary/.test(ci) || !/sha256sum --check SHA256SUMS/.test(ci) || !/REPOMEMO_WEB_DIR="\$home\/web"/.test(ci)) problems.push(".github/workflows/ci.yml does not pack the server, unpack it clean and start it on its own web app");
}

// The upgrade and rollback guide for direct installs (V0.4.36) begins with a backup and says what the refusal means.
{
  const guide = read("docs/DIRECT_UPGRADE.md");
  const upgrading = guide.slice(guide.indexOf("## Upgrading"), guide.indexOf("## Rolling back"));
  if (!/^1\. \*\*Back up/m.test(upgrading)) problems.push("docs/DIRECT_UPGRADE.md does not begin the upgrade with a backup");
  for (const needle of ["ln -sfn /opt/repomemo-<old> /opt/repomemo", "New-Item -ItemType Junction", "repomemo.version", "newer than this program", "Nothing\n     was changed", "Restore the backup"]) {
    if (!guide.includes(needle)) problems.push(`docs/DIRECT_UPGRADE.md lacks ${needle.replace(/\n/g, " ")}`);
  }
  const newer = read("crates/storage/src/newer.rs");
  if (!newer.includes("newer than this program") || !newer.includes("Nothing was changed") || !newer.includes("MARKER_FILE")) problems.push("crates/storage/src/newer.rs no longer refuses a newer database with a message that names both versions");
  if (!read("crates/storage/src/lib.rs").includes("newer::refuse_newer_database(&pool, &db_dir).await?;\n        MIGRATOR.run")) problems.push("the storage engine no longer refuses a newer database before it migrates");
}

// The server is built for three platforms and attached to the release (V0.4.35): native runners, the tag must be the
// version, one SHA256SUMS, and nothing is attached by a pull request or a branch.
{
  const archives = workflows.find(([name]) => name === "server-archives.yml")?.[1] ?? "";
  if (!archives) problems.push(".github/workflows/server-archives.yml is missing");
  else {
    for (const platform of ["os: linux, arch: x64, runner: ubuntu-latest", "os: linux, arch: arm64, runner: ubuntu-24.04-arm", "os: windows, arch: x64, runner: windows-latest"]) {
      if (!archives.includes(platform)) problems.push(`the server archives are not built for ${platform}`);
    }
    if (!/tags:\s*\["v\*"\]/.test(archives) || /pull_request/.test(archives)) problems.push("the archives workflow must run on a v* tag (or by hand) and never on a pull request");
    if (!/test "\$\{GITHUB_REF_NAME\}" = "v\$\(node -p/.test(archives)) problems.push("the archives workflow does not require the tag to be the version");
    if (!/^  release:\n[\s\S]*?if: startsWith\(github\.ref, 'refs\/tags\/v'\)/m.test(archives) || !/contents: write/.test(archives)) problems.push("only a tag may attach archives to a release, and only that job may write");
    if (!/sha256sum --check SHA256SUMS/.test(archives) || !/= "3"/.test(archives) || !/gh release upload/.test(archives)) problems.push("the archives workflow does not check three archives against one SHA256SUMS before attaching them");
    if (/^permissions:\s*\n\s+contents:\s*write/m.test(archives)) problems.push("the archives workflow gives every job write access");
  }
}

// The images on the registry (V0.4.37): a v* tag runs the checks, builds the four images natively for both architectures and
// gives each one name under X.Y.Z, X.Y and latest. A pull request or a branch publishes nothing, and only the two jobs that
// push may write packages.
{
  const release = workflows.find(([name]) => name === "release-images.yml")?.[1] ?? "";
  if (!release) problems.push(".github/workflows/release-images.yml is missing");
  else {
    if (!/tags:\s*\["v\*"\]/.test(release) || /pull_request/.test(release)) problems.push("the release images workflow must run on a v* tag (or by hand) and never on a pull request");
    if (!/test "\$\{GITHUB_REF_NAME\}" = "v\$\(node -p/.test(release)) problems.push("the release images workflow does not require the tag to be the version");
    for (const check of ["set-version.mjs --check", "check-container.mjs", "check-migrations.mjs", "check-docs.mjs", "check-templates.mjs"]) {
      if (!release.includes(check)) problems.push(`the release images workflow does not run ${check} before building`);
    }
    for (const arch of ["{ name: amd64, runner: ubuntu-latest }", "{ name: arm64, runner: ubuntu-24.04-arm }"]) {
      if (!release.includes(arch)) problems.push(`the release images are not built natively for ${arch}`);
    }
    for (const image of ["{ name: server, target: server }", "{ name: converter, target: converter }", "{ name: edge, target: edge }", "{ name: web, target: webapp }"]) {
      if (!release.includes(image)) problems.push(`the release images workflow does not build ${image}`);
    }
    if (!release.includes('--tag "${name}:${version}" --tag "${name}:${minor}" --tag "${name}:latest"')) problems.push("the release images are not named X.Y.Z, X.Y and latest");
    if (!release.includes('if [ "${GITHUB_REF_TYPE}" = "tag" ]') || !/^  manifest:\n[\s\S]*?if: startsWith\(github\.ref, 'refs\/tags\/v'\)/m.test(release)) problems.push("only a tag may push an image or name it");
    if (/^permissions:\s*\n\s+(contents|packages):\s*write/m.test(release) || (release.match(/packages: write/g) ?? []).length !== 2) problems.push("only the build and manifest jobs may write packages");
  }
}

// The compose file runs released images (V0.4.40): it builds nothing, names the four images on the registry with the release's
// tag, and the build file is the one place a build is described. CI builds through it and checks the two readings of the files.
{
  if (/^\s+build:/m.test(compose)) problems.push("docker-compose.yml builds an image; the build belongs to docker-compose.build.yml");
  for (const name of ["server", "converter", "web", "edge"]) {
    if (!compose.includes(`/repomemo-${name}:\${REPOMEMO_VERSION:-latest}`) || !compose.includes("${REPOMEMO_REGISTRY:-ghcr.io/repomemo}")) problems.push(`docker-compose.yml does not run the released ${name} image from the registry`);
    if (!new RegExp(`image:.*repomemo-${name}:\\$\\{REPOMEMO_VERSION:-local\\}`).test(composeBuild)) problems.push(`docker-compose.build.yml does not tag the ${name} image it builds with a local name`);
  }
  if (!/^docker-compose\.build\.yml text eol=lf/m.test(attributes)) problems.push(".gitattributes does not keep docker-compose.build.yml on LF line endings");
  if (!/^COMPOSE_FILE: docker-compose\.yml:docker-compose\.build\.yml/m.test(ci.replace(/^ +/gm, ""))) problems.push("CI does not build the images of the commit through the build file (COMPOSE_FILE)");
  if (!/check-compose-images\.mjs released/.test(ci) || !/check-compose-images\.mjs built/.test(ci)) problems.push("CI does not check both readings of the compose files (released and built)");
  if (!/^REPOMEMO_VERSION=(latest|\d+\.\d+\.\d+)$/m.test(example)) problems.push(".env.example does not name a released tag (latest or X.Y.Z) in REPOMEMO_VERSION");
}

// The quick start (V0.4.41): the container files of a release are packed from a list, attached with the script, and listed in the
// release's SHA256SUMS; the script checks the archive against them before it unpacks anything. CI tries all of it.
{
  const list = read("deploy/compose-bundle.txt").split(/\r?\n/).filter((line) => line && !line.startsWith("#"));
  for (const path of list) if (!existsSync(path)) problems.push(`deploy/compose-bundle.txt names ${path}, which is not there`);
  for (const path of ["docker-compose.yml", ".env.example", "deploy/init.sh", "deploy/init.ps1", "deploy/upgrade.sh"]) {
    if (!list.includes(path)) problems.push(`deploy/compose-bundle.txt leaves out ${path}`);
  }
  for (const match of compose.matchAll(/^\s+-\s+(?:\$\{[A-Z_]+:-)?\.\/(deploy\/[\w./-]+)/gm)) {
    if (!list.includes(match[1]) && !/metrics-token$/.test(match[1]) && !/certs$/.test(match[1])) problems.push(`the compose file mounts ${match[1]}, which the container bundle leaves out`);
  }
  const script = read("deploy/quickstart.sh");
  if (!/sha256sum|shasum/.test(script) || !/nothing was unpacked/.test(script) || script.indexOf("checksum of") > script.indexOf("tar --extract")) problems.push("deploy/quickstart.sh does not check the download before it unpacks it");
  if (!/REPOMEMO_VERSION=\$version/.test(script)) problems.push("deploy/quickstart.sh does not pin the install to the release it fetched");
  const archives = workflows.find(([name]) => name === "server-archives.yml")?.[1] ?? "";
  if (!/package-compose\.sh/.test(archives) || !/repomemo-compose-\*\.tar\.gz quickstart\.sh \| sort/.test(archives) || !/archives\/quickstart\.sh archives\/SHA256SUMS/.test(archives)) problems.push("the release does not attach the container files, the quick start script and their sums");
  if (!/deploy\/quickstart\.sh --version 9\.9\.9 --base-url/.test(ci) || !/does not match its sums unpacks nothing/.test(ci)) problems.push("CI does not try the quick start, including a download that does not match its sums");
  if (!existsSync("docs/QUICK_START.md")) problems.push("docs/QUICK_START.md is missing");
}

// The administrator's command for the desktop app (V0.4.48): it checks the address by the app's own rules, compiles it in when it
// builds, and with --from needs no Rust; CI tries it.
{
  const command = read("scripts/package-desktop.mjs");
  if (!command.includes("REPOMEMO_PRESET_SERVER") || !command.includes('arg("from")') || !command.includes("repomemo-desktop.json")) problems.push("scripts/package-desktop.mjs must compile the address in, take --from for an administrator without Rust, and write repomemo-desktop.json");
  const packageJson = read("package.json");
  if (!/"desktop:package": "node scripts\/package-desktop\.mjs"/.test(packageJson)) problems.push("package.json has no desktop:package script");
  if (!/node --test scripts\/package-desktop\.test\.mjs/.test(ci)) problems.push("CI does not run the desktop packaging tests");
}

// The desktop app for Windows is built in CI (V0.4.49): the per-person installer (WebView2 fetched when missing), one that carries
// WebView2, the portable executable, started once, kept as an artifact and published nowhere.
{
  const windows = workflows.find(([name]) => name === "desktop-windows.yml")?.[1] ?? "";
  if (!windows) problems.push(".github/workflows/desktop-windows.yml is missing");
  else {
    if (!/runs-on: windows-latest/.test(windows) || !/npx tauri build --bundles nsis\b/.test(windows)) problems.push("the Windows desktop workflow does not build the installer on windows-latest");
    if (!/tauri\.offline\.conf\.json/.test(windows) || !/"type": "offlineInstaller"/.test(read("apps/desktop/src-tauri/tauri.offline.conf.json"))) problems.push("the Windows desktop workflow does not build the installer that carries WebView2");
    if (!/"webviewInstallMode":\s*\{\s*"type":\s*"downloadBootstrapper"/.test(read("apps/desktop/src-tauri/tauri.conf.json"))) problems.push("the installer does not fetch WebView2 when it is missing (webviewInstallMode downloadBootstrapper)");
    if (!/repomemo-desktop\.exe/.test(windows) || !/MainWindowTitle/.test(windows)) problems.push("the Windows desktop workflow neither keeps the portable executable nor starts it to see it open");
    if (!/upload-artifact@[0-9a-f]{40}[\s\S]*desktop-windows-x64/.test(windows)) problems.push("the Windows desktop workflow does not keep its files as the artifact desktop-windows-x64");
    // Only the update job of V0.4.56 may write, on a tag, once the signing key's public half is set.
    const buildPart = windows.slice(0, windows.indexOf("\n  publish-update:") > 0 ? windows.indexOf("\n  publish-update:") : undefined);
    if (/contents:\s*write|gh release|packages:\s*write/.test(buildPart)) problems.push("the Windows desktop build publishes or writes; it only keeps an artifact");
    if (!/test "\$\{GITHUB_REF_NAME\}" = "v\$\(node -p/.test(windows)) problems.push("the Windows desktop workflow does not require the tag to be the version");
  }
}

// The Windows installer (V0.4.50): per person with no administrator rights, silent with /S, shortcuts, and an uninstaller that
// offers to forget the saved session by asking the program itself, which has a flag for it and opens no window; CI installs,
// opens and removes it.
{
  const shell = read("apps/desktop/src-tauri/tauri.conf.json");
  if (!/"installMode":\s*"currentUser"/.test(shell)) problems.push("the Windows installer is not per person (nsis installMode currentUser)");
  if (!/"installerHooks":\s*"windows\/hooks\.nsh"/.test(shell)) problems.push("the Windows installer has no hooks (installerHooks windows/hooks.nsh)");
  const hooks = read("apps/desktop/src-tauri/windows/hooks.nsh");
  if (!/NSIS_HOOK_PREUNINSTALL/.test(hooks) || !/--forget-session/.test(hooks) || !/IfSilent keep_session/.test(hooks) || !/MessageBox MB_YESNO/.test(hooks)) problems.push("the uninstaller does not offer to forget the saved session (and keep it on a silent removal)");
  const connection = read("apps/desktop/src-tauri/src/connection.rs");
  if (!connection.includes('"--forget-session"')) problems.push("the program does not take --forget-session, which the uninstaller runs");
  if (!/windows_subsystem = "windows"/.test(read("apps/desktop/src-tauri/src/main.rs"))) problems.push("a release build opens a console window behind the app (windows_subsystem)");
  const windows = workflows.find(([name]) => name === "desktop-windows.yml")?.[1] ?? "";
  for (const needle of ['ArgumentList "/S"', "LOCALAPPDATA", "HKCU:", "uninstall.exe", "Start Menu"]) {
    if (!windows.includes(needle)) problems.push(`the Windows desktop workflow does not try the installer (${needle})`);
  }
}

// The app's identity (V0.4.51): an icon set made from the logo, the product's details in the executable, one copy per person, the
// window's size and place remembered, and the title bar following the page; CI looks at the executable and starts a second copy.
{
  const shell = read("apps/desktop/src-tauri/tauri.conf.json");
  for (const icon of ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]) {
    if (!shell.includes(`"${icon}"`) || !existsSync(`apps/desktop/src-tauri/${icon}`)) problems.push(`the desktop icon set lacks ${icon}`);
  }
  for (const key of ["publisher", "copyright", "shortDescription", "longDescription", "homepage", "category"]) {
    if (!new RegExp(`"${key}":\\s*"[^"]+"`).test(shell)) problems.push(`tauri.conf.json gives the executable no ${key}`);
  }
  const shellCode = read("apps/desktop/src-tauri/src/lib.rs");
  if (!/tauri_plugin_single_instance::init/.test(shellCode) || !/asked_to_forget/.test(shellCode.slice(0, shellCode.indexOf("tauri_plugin_single_instance")))) problems.push("the app is not limited to one copy, or the uninstaller's start is held up by it");
  if (!/tauri_plugin_window_state::Builder/.test(shellCode)) problems.push("the window does not come back where it was (tauri-plugin-window-state)");
  if (!/core:window:allow-set-title/.test(read("apps/desktop/src-tauri/capabilities/default.json")) || !/followPageTitle/.test(read("apps/desktop/src/components/DesktopApp.tsx"))) problems.push("the window's title does not follow the page's title");
  const windows = workflows.find(([name]) => name === "desktop-windows.yml")?.[1] ?? "";
  if (!/VersionInfo/.test(windows) || !/a second copy of the app stayed open/.test(windows)) problems.push("the Windows desktop workflow does not look at the executable's details or start a second copy");
}

// Native notices (V0.4.53): the shell shows a notification and a badge only when asked, the client decides what to tell, and a
// browser has no notifier.
{
  const shellCode = read("apps/desktop/src-tauri/src/lib.rs");
  if (!/tauri_plugin_notification::init/.test(shellCode) || !/notice::notify/.test(shellCode) || !/notice::set_badge/.test(shellCode)) problems.push("the shell does not register the notification plugin and the notice and badge commands");
  const watcher = read("apps/desktop/src/web/NoticeWatcher.tsx");
  if (!/currentNotifier\(\)/.test(watcher) || !/windowIsInBackground\(\)/.test(watcher) || !/badge\(0\)/.test(watcher)) problems.push("the notice watcher must do nothing without a notifier, notify only from the background and take the badge away when it stops");
  if (!/nativeNotices \? <NoticeWatcher/.test(read("apps/desktop/src/SharedWebApp.tsx"))) problems.push("the notice watcher is not shown by capability (platform.nativeNotices)");
}

// repomemo:// links (V0.4.54): the system hands them to the one running copy, the shell takes only the client's own paths for its own
// server, the installer registers the scheme for the person, and the page offers them in a browser only.
{
  const shell = read("apps/desktop/src-tauri/tauri.conf.json");
  if (!/"deep-link":\s*\{\s*"desktop":\s*\{\s*"schemes":\s*\["repomemo"\]/.test(shell)) problems.push("tauri.conf.json does not register the repomemo scheme (plugins deep-link)");
  if (!/tauri-plugin-single-instance = \{[^}]*"deep-link"/.test(read("Cargo.toml"))) problems.push("the single-instance plugin does not pass a link to the running copy (feature deep-link)");
  const links = read("apps/desktop/src-tauri/src/deeplink.rs");
  if (!/fn is_app_path/.test(links) || !/connected to/.test(links) || !/APP_AREAS/.test(links)) problems.push("the shell does not check a link's path and server");
  if (!/take_pending_link/.test(read("apps/desktop/src-tauri/src/lib.rs")) || !/on_open_url/.test(read("apps/desktop/src-tauri/src/lib.rs"))) problems.push("the shell does not hand a link to the page (on_open_url, take_pending_link)");
  const windows = workflows.find(([name]) => name === "desktop-windows.yml")?.[1] ?? "";
  if (!/Classes\\repomemo\\shell\\open\\command/.test(windows)) problems.push("the Windows desktop workflow does not check that the repomemo:// links are registered for the person");
  if (!/opensInApp/.test(read("apps/desktop/src/components/OpenInAppLink.tsx"))) problems.push("Open in the app is not shown by capability (platform.opensInApp)");
}

// A server with its own certificate authority (V0.4.55): the bundle is trusted for the file's server only, by a loopback relay that
// uses no other root, forwards to one origin and answers nothing without the run's secret; the window may call only the relay.
{
  const relay = read("apps/desktop/src-tauri/src/trust.rs");
  if (!/tls_built_in_root_certs\(false\)/.test(relay) || !/add_root_certificate/.test(relay)) problems.push("the relay must trust the administrator's bundle alone (tls_built_in_root_certs(false) and add_root_certificate)");
  if (!/TOKEN_HEADER/.test(relay) || !/StatusCode::FORBIDDEN/.test(relay) || !/upstream_url\(&origin/.test(relay)) problems.push("the relay must answer nothing without the run's secret and forward to its one origin");
  if (!/Policy::none\(\)/.test(relay) || !/127\.0\.0\.1/.test(relay)) problems.push("the relay must follow no redirect and listen on the loopback only");
  const connection = read("apps/desktop/src-tauri/src/connection.rs");
  if (!/fn uses_private_ca/.test(connection) || !/== Some\(origin\)/.test(connection) || !/allow_address\(app, &relay\.proxy\)/.test(connection)) problems.push("the bundle is not tied to the file's server, or the window may call the real server beside the relay");
  if (!/certificate: Option<String>/.test(read("apps/desktop/src-tauri/src/config.rs"))) problems.push("the configuration file takes no certificate");
  if (!/throughRelay/.test(read("apps/desktop/src/lib/transport.ts")) || !/desktopServer\.route\(\)/.test(read("apps/desktop/src/lib/desktopConnection.ts"))) problems.push("the client does not send a private-authority server's requests through the relay");
  if (!existsSync("apps/desktop/src-tauri/testdata/test-ca.pem") || /PRIVATE KEY/.test(read("apps/desktop/src-tauri/testdata/test-ca.pem"))) problems.push("the test certificate is missing, or holds a private key");
}

// Signed updates (V0.4.56): the app checks a feed whose updates are signed (the plugin refuses an unsigned one, and a signature made for
// another version), only the administrator's feed or the person's own choice makes it look, and only a tag with the signing key
// publishes one.
{
  const shell = read("apps/desktop/src-tauri/tauri.conf.json");
  if (!/"requireSignedVersion":\s*true/.test(shell) || !/"updater":/.test(shell)) problems.push("the updater does not require the signed version (a replayed older release)");
  if (/dangerous/i.test(shell) || /dangerous/i.test(read("apps/desktop/src-tauri/src/updates.rs"))) problems.push("the updater is told to accept something dangerous (insecure transport, invalid certificates)");
  const updates = read("apps/desktop/src-tauri/src/updates.rs");
  if (!/Mode::Optional && !enabled/.test(updates) || !/Mode::Managed/.test(updates)) problems.push("the app must look for the project's updates only when the person chose, and for the administrator's always");
  if (!/updates::check_update/.test(read("apps/desktop/src-tauri/src/lib.rs"))) problems.push("the shell does not register the update commands");
  const windows = workflows.find(([name]) => name === "desktop-windows.yml")?.[1] ?? "";
  const job = windows.slice(windows.indexOf("\n  publish-update:"));
  if (!/if: startsWith\(github\.ref, 'refs\/tags\/v'\) && vars\.REPOMEMO_UPDATE_PUBKEY != ''/.test(job) || !/contents: write/.test(job) || !/update-feed\.mjs/.test(job)) problems.push("only a tag with the update key may publish the signed update feed");
  if (!/feed:test|update-feed\.test\.mjs/.test(ci)) problems.push("CI does not run the update feed tests");
  if (!/"feed:test":/.test(read("package.json"))) problems.push("package.json has no feed:test script");
}

// Windows code signing (V0.4.57): the certificate is read by the one step that imports it, the build signs with a timestamp, the
// job fails when a signed build's files are not validly signed, and an unsigned build says so.
{
  const windows = workflows.find(([name]) => name === "desktop-windows.yml")?.[1] ?? "";
  if (!/HAS_WINDOWS_CERT:\s*\$\{\{\s*secrets\.WINDOWS_CERTIFICATE != ''/.test(windows)) problems.push("the Windows desktop workflow does not sign only when the repository has a certificate");
  const importStep = /- name: Import the signing certificate[\s\S]*?(?=\n      - name:)/.exec(windows)?.[0] ?? "";
  if (!/secrets\.WINDOWS_CERTIFICATE\b/.test(importStep) || !/Remove-Item \$pfx/.test(importStep)) problems.push("the signing certificate must be read, and the file removed, in the step that imports it");
  if ((windows.match(/secrets\.WINDOWS_CERTIFICATE\b/g) ?? []).length !== 2) problems.push("the signing certificate is used outside its import step");
  if (!/certificateThumbprint/.test(windows) || !/digestAlgorithm = "sha256"/.test(windows) || !/timestampUrl/.test(windows)) problems.push("the build must sign with the certificate's thumbprint, SHA-256 and a timestamp");
  if ((windows.match(/tauri\.signing\.conf\.json/g) ?? []).length < 3) problems.push("both installers must be built with the signing configuration");
  if (!/Get-AuthenticodeSignature/.test(windows) || !/is not validly signed/.test(windows) || !/is not signed \(the repository has no WINDOWS_CERTIFICATE secret\)/.test(windows)) problems.push("the workflow must fail on a bad signature and say plainly that a build is unsigned");
  if (!/tauri\.signing\.conf\.json/.test(read(".gitignore"))) problems.push(".gitignore does not keep the generated signing configuration out");
  if (!/SmartScreen/.test(read("docs/DEVELOPMENT_COMMANDS.md"))) problems.push("the SmartScreen warning is not explained");
}

// The parity check of the desktop app (V0.4.58) runs in CI with its tests, and its list exists.
{
  if (!/node scripts\/check-desktop\.mjs/.test(ci) || !/check-desktop\.test\.mjs/.test(ci)) problems.push("CI does not run the desktop parity check and its tests");
  if (!/"desktop:check": "node scripts\/check-desktop\.mjs"/.test(read("package.json"))) problems.push("package.json has no desktop:check script");
  if (!existsSync("apps/desktop/desktop-only.json")) problems.push("apps/desktop/desktop-only.json (the list of what is for the desktop alone) is missing");
}

// The page of a release is written from the changelog (V0.4.39): the workflow that makes the release uses the script, never a
// sentence typed into it, and the script lists what a release holds.
{
  const archives = workflows.find(([name]) => name === "server-archives.yml")?.[1] ?? "";
  if (archives) {
    if (!/node scripts\/release-notes\.mjs --version/.test(archives) || !/--notes-file release-notes\.md/.test(archives) || /--notes "/.test(archives)) problems.push("the release page must be written by scripts/release-notes.mjs from the changelog, not typed into the workflow");
    if (!/gh release edit .* --notes-file/.test(archives)) problems.push("a re-run must rewrite the release page from the changelog too");
  }
  const notes = read("scripts/release-notes.mjs");
  for (const needle of ["repomemo-${name}:${version}", "SHA256SUMS", "service/install.sh", "install.ps1", "VERIFYING_IMAGES.md"]) {
    if (!notes.includes(needle)) problems.push(`scripts/release-notes.mjs does not list ${needle}`);
  }
  if (!/release-notes\.test\.mjs/.test(workflows.find(([name]) => name === "ci.yml")?.[1] ?? "")) problems.push("CI does not run the release notes tests");
}

// Provenance (V0.4.38): each joined image gets a signed build attestation and a signed SBOM, kept with the image, and the
// page that says how to verify them exists.
{
  const release = workflows.find(([name]) => name === "release-images.yml")?.[1] ?? "";
  if (release) {
    if (!/uses: actions\/attest-build-provenance@[0-9a-f]{40}/.test(release) || !/uses: actions\/attest-sbom@[0-9a-f]{40}/.test(release)) problems.push("the release images workflow does not attest the build and the SBOM");
    if ((release.match(/push-to-registry: true/g) ?? []).length !== 2) problems.push("the attestations are not kept with the image on the registry");
    if (!/spdx-json/.test(release) || !/TRIVY_IMAGE: ghcr\.io\/aquasecurity\/trivy:[^@\s]+@sha256:[0-9a-f]{64}/.test(release)) problems.push("the SBOM is not made by a scanner pinned by digest");
    const manifestJob = release.slice(release.indexOf("\n  manifest:"));
    if (!/id-token: write/.test(manifestJob) || !/attestations: write/.test(manifestJob) || (release.match(/id-token: write/g) ?? []).length !== 1) problems.push("only the manifest job may sign (id-token and attestations write)");
  }
  const guide = read("docs/VERIFYING_IMAGES.md");
  for (const needle of ["gh attestation verify", "cosign verify-attestation", "spdx.dev/Spdx", "by digest"]) {
    if (!guide.includes(needle)) problems.push(`docs/VERIFYING_IMAGES.md lacks ${needle}`);
  }
}

// Front doors for a direct install (V0.4.34): one page, a Caddyfile and an nginx file for the server on 127.0.0.1:3020, both
// validated in CI by the programs themselves.
{
  const page = read("docs/FRONT_DOORS.md");
  const caddy = read("deploy/direct/Caddyfile");
  const nginx = read("deploy/direct/nginx.conf");
  for (const needle of ["REPOMEMO_PUBLIC_URL", "REPOMEMO_ALLOWED_ORIGIN", "REPOMEMO_TRUST_PROXY", "deploy/direct/Caddyfile", "deploy/direct/nginx.conf", "ufw allow", "firewall-cmd", "New-NetFirewallRule", "certbot", "Do **not** open 3020"]) {
    if (!page.includes(needle)) problems.push(`docs/FRONT_DOORS.md lacks ${needle}`);
  }
  if (!caddy.includes("reverse_proxy 127.0.0.1:3020") || !caddy.includes("flush_interval -1")) problems.push("deploy/direct/Caddyfile does not send everything to the server with the event streams unbuffered");
  if (!nginx.includes("proxy_pass http://127.0.0.1:3020") || !/proxy_buffering off;/.test(nginx) || !/Strict-Transport-Security/.test(nginx)) problems.push("deploy/direct/nginx.conf does not proxy to the server, unbuffer the event streams and send HSTS");
  if (!/caddy validate --config/.test(ci) || !/nginx -t/.test(ci) || !/^  direct-front-doors:/m.test(ci)) problems.push(".github/workflows/ci.yml does not have Caddy and nginx validate the direct-install samples");
}

// Windows: the server runs as a service, registered by its own command, with a script that is safe to run twice (V0.4.33).
{
  const install = read("deploy/direct/service/install.ps1");
  const uninstall = read("deploy/direct/service/uninstall.ps1");
  const service = read("apps/server/src/winservice.rs");
  for (const needle of ["service install --workdir", "exists; leaving it as it is", "Junction", "icacls", "is already registered", "GenerateSecrets", "never overwritten"]) {
    if (!install.includes(needle)) problems.push(`deploy/direct/service/install.ps1 lacks ${needle}`);
  }
  if (!/service uninstall/.test(uninstall) || /Remove-Item[^\n]*ProgramData/i.test(uninstall)) problems.push("deploy/direct/service/uninstall.ps1 must unregister the service and never delete the data folder");
  for (const needle of ["ServiceStartType::AutoStart", "NT AUTHORITY\\\\LocalService", "ServiceControl::Stop", "run_server", "restart/5000"]) {
    if (!service.includes(needle)) problems.push(`apps/server/src/winservice.rs lacks ${needle}`);
  }
  if (!/^\[target\.'cfg\(windows\)'\.dependencies\]\s*\nwindows-service/m.test(serverManifest)) problems.push("apps/server/Cargo.toml does not depend on windows-service for Windows only");
  if (!/^  server-archive-windows:/m.test(ci) || !/install\.ps1"? -GenerateSecrets -Start/.test(ci) || !/StartType -ne "Automatic"/.test(ci) || !/uninstall\.ps1/.test(ci)) problems.push(".github/workflows/ci.yml does not install, start, stop and remove the Windows service");
}

// Linux: a hardened systemd unit and an install script that is safe to run twice (V0.4.32).
{
  const unit = read("deploy/direct/service/repomemo.service");
  const install = read("deploy/direct/service/install.sh");
  for (const directive of ["User=repomemo", "NoNewPrivileges=true", "CapabilityBoundingSet=", "ProtectSystem=strict", "ReadWritePaths=/var/lib/repomemo", "PrivateTmp=true", "ProtectHome=true", "RestrictAddressFamilies=", "SystemCallFilter=@system-service", "MemoryDenyWriteExecute=true", "EnvironmentFile=/etc/repomemo/repomemo.env", "ExecStart=/opt/repomemo/bin/repomemo-server", "TimeoutStopSec=", "KillSignal=SIGTERM"]) {
    if (!unit.includes(directive)) problems.push(`deploy/direct/service/repomemo.service lacks ${directive}`);
  }
  const stop = Number(/^TimeoutStopSec=(\d+)/m.exec(unit)?.[1] ?? 0);
  if (stop <= 30) problems.push("the unit's TimeoutStopSec is not longer than the server's shutdown grace of 30 seconds");
  for (const needle of ["useradd --system", "getent passwd repomemo", "ln -sfn", "cmp -s", "chmod 0600", "exists; leaving it as it is", "systemctl daemon-reload", "systemctl enable"]) {
    if (!install.includes(needle)) problems.push(`deploy/direct/service/install.sh does not do what makes it safe to run twice: ${needle}`);
  }
  if (/\brm -rf "?\$prefix\/repomemo"?\b|rm -rf \/var\/lib/.test(install)) problems.push("deploy/direct/service/install.sh deletes the data or the link target");
  const env = read("deploy/direct/repomemo.env.example");
  if (!env.includes("REPOMEMO_WEB_DIR=/opt/repomemo/web")) problems.push("the settings example's web folder is not under the link the unit runs from (/opt/repomemo)");
  if (!/systemd-analyze verify \/etc\/systemd\/system\/repomemo\.service/.test(ci) || !/service\/install\.sh" --generate-secrets --start/.test(ci) || !/A second run changes nothing/.test(ci)) problems.push(".github/workflows/ci.yml does not verify the unit, install the archive as a service and run the installer twice");
}

// Metrics in containers (V0.4.30): an example, behind its own profile, pinned, on the loopback, read-only, with the token
// mounted read-only and the scrape configuration reading it; CI shows the figures.
{
  const block = /^  prometheus:\n([\s\S]*?)(?=^  \S|^\S|(?![\s\S]))/m.exec(compose)?.[1] ?? "";
  if (!/^\s+profiles:\s*\n\s+-\s*monitoring\s*$/m.test(block)) problems.push("docker-compose.yml: Prometheus is not behind the monitoring profile, so it would start by default");
  if (!/image:\s*prom\/prometheus:v[\d.]+@sha256:[0-9a-f]{64}/.test(block)) problems.push("docker-compose.yml: the Prometheus image is not pinned by tag and digest");
  if (!/"127\.0\.0\.1:9090:9090"/.test(block)) problems.push("docker-compose.yml: Prometheus is published beyond the loopback");
  if (!/^\s+read_only:\s*true\s*$/m.test(block) || !/metrics-token\}?:\/etc\/prometheus\/metrics-token:ro/.test(block)) problems.push("docker-compose.yml: Prometheus is not read-only or does not mount the token read-only");
  const scrape = read("deploy/monitoring/prometheus.yml");
  if (!scrape.includes("/v1/system/metrics") || !scrape.includes("credentials_file: /etc/prometheus/metrics-token") || !scrape.includes("server:3020")) problems.push("deploy/monitoring/prometheus.yml does not scrape the server's metrics with the token");
  if (!/^deploy\/monitoring\/metrics-token$/m.test(read(".gitignore"))) problems.push(".gitignore does not keep the metrics token out of the repository");
  if (!/^  monitoring:/m.test(ci) || !/query=repomemo_users/.test(ci)) problems.push(".github/workflows/ci.yml does not start the monitoring example and read a figure from it");
  if (!readFileSync("apps/server/src/system.rs", "utf8").includes("repomemo_users")) problems.push("the server no longer has the repomemo_users metric the example reads");
}

// The server logs JSON to standard output and keeps no log files in a container (V0.4.29); the lines written outside the
// logging (the entrypoint, a refused start) have the same shape.
for (const [file, text] of [["docker-compose.yml", compose], ["deploy/api.yml", read("deploy/api.yml")]]) {
  if (!/REPOMEMO_LOG_FORMAT:\s*json\b/.test(text) || !/REPOMEMO_LOG_TO_FILE:\s*"?false"?/.test(text)) problems.push(`${file} does not make the server log JSON to standard output with no log files`);
}
if (!/say\(\)/.test(read("docker/entrypoint.sh")) || !/"target":"repomemo_entrypoint"/.test(read("docker/entrypoint.sh"))) problems.push("docker/entrypoint.sh does not write its lines as JSON when REPOMEMO_LOG_FORMAT is json");
if (!/docker compose logs --no-log-prefix[^\n]*server/.test(ci) || !/JSON\.parse\(line\)/.test(ci)) problems.push(".github/workflows/ci.yml does not check that every line of the server's log is JSON");

// Every service can say it is healthy, stops with time to finish, restarts and rotates its logs (V0.4.28), so that
// `docker compose up --wait` returns only when the whole stack is.
{
  const stageHasHealthcheck = (target) => {
    const start = dockerfile.search(new RegExp(String.raw`^FROM\s.*\sAS\s+${target}\s*$`, "m"));
    const next = dockerfile.slice(start + 1).search(/^FROM\s/m);
    return start >= 0 && /^HEALTHCHECK\b/m.test(next < 0 ? dockerfile.slice(start) : dockerfile.slice(start, start + 1 + next));
  };
  for (const [file, text] of [["docker-compose.yml", compose], ["deploy/api.yml", read("deploy/api.yml")]]) {
    const body = text.slice(text.indexOf("\nservices:"), text.indexOf("\nnetworks:") > 0 ? text.indexOf("\nnetworks:") : text.indexOf("\nvolumes:"));
    for (const match of body.matchAll(/^  ([a-z][\w-]*):\n([\s\S]*?)(?=^  \S|(?![\s\S]))/gm)) {
      const [, name, block] = match;
      const target = /target:\s*(\S+)/.exec(block)?.[1] ?? new RegExp(String.raw`^  ${name}:\n    build:\n      context: \.\n      target: (\S+)`, "m").exec(composeBuild)?.[1];
      if (!/^\s+healthcheck:/m.test(block) && !(target && stageHasHealthcheck(target))) problems.push(`${file}: ${name} has no health check, in the compose file or in its image`);
      if (!/^\s+restart:\s*unless-stopped\s*$/m.test(block)) problems.push(`${file}: ${name} does not restart unless stopped`);
      if (!/^\s+stop_grace_period:\s*\d+s\s*$/m.test(block)) problems.push(`${file}: ${name} has no stop_grace_period`);
      if (!/^\s+logging:\s*\*logging\s*$/m.test(block)) problems.push(`${file}: ${name} does not rotate its logs`);
    }
    if (!/^x-logging:\s*&logging\s*\n\s+driver:\s*json-file\s*\n\s+options:\s*\n\s+max-size:\s*"\d+m"\s*\n\s+max-file:\s*"\d+"/m.test(text)) problems.push(`${file} has no log rotation (x-logging)`);
  }
  // The server gets longer to stop than the grace it gives running requests (REPOMEMO_SHUTDOWN_GRACE_SECONDS, 30 by default).
  const grace = Number(/^  server:\n[\s\S]*?stop_grace_period:\s*(\d+)s/m.exec(compose)?.[1] ?? 0);
  if (grace <= 30) problems.push("the server's stop_grace_period is not longer than its shutdown grace of 30 seconds");
  for (const name of ["web", "edge"]) {
    if (!new RegExp(`^  ${name}:\\n[\\s\\S]*?depends_on:\\s*\\n\\s+\\w+:\\s*\\n\\s+condition:\\s*service_healthy`, "m").test(compose)) problems.push(`docker-compose.yml: ${name} does not wait for the service it fronts to be healthy`);
  }
  if (!/docker compose ps --format[^\n]*Health/.test(ci)) problems.push(".github/workflows/ci.yml does not read the health of every service after --wait");
}

// The previous release's data survives this one (V0.4.27): the job builds the release named in deploy/upgrade-from.txt,
// seeds it, moves its volume and checks everything through the new stack.
{
  const from = read("deploy/upgrade-from.txt");
  if (!/^[0-9a-f]{40}\s/m.test(from.replace(/^#.*$/gm, ""))) problems.push("deploy/upgrade-from.txt names no commit (40 hexadecimal characters) for the upgrade test to start from");
  const job = /^  upgrade-from-previous:\n([\s\S]*?)(?=^  \S|(?![\s\S]))/m.exec(ci)?.[1] ?? "";
  if (!job) problems.push(".github/workflows/ci.yml has no upgrade-from-previous job");
  else {
    for (const needle of ["deploy/upgrade-from.txt", "ref: ${{ env.PREVIOUS_SHA }}", "upgrade-test.mjs seed", "migrate-volumes.sh", "upgrade-test.mjs verify"]) {
      if (!job.includes(needle)) problems.push(`the upgrade-from-previous job lacks ${needle}`);
    }
  }
  const script = read("scripts/upgrade-test.mjs");
  for (const call of ["/v1/setup/admin", "/artifacts/upload", "/search", "/v1/auth/login", "/file"]) {
    if (!script.includes(call)) problems.push(`scripts/upgrade-test.mjs does not use ${call}`);
  }
}

// The compose job runs the smoke test over the whole stack (V0.4.26), setup included, and the script does the first hour.
{
  const smoke = read("scripts/smoke-test.mjs");
  if (!/REPOMEMO_SMOKE_SETUP_CODE=\S+\s+node scripts\/smoke-test\.mjs https:\/\/localhost/.test(ci) || !/echo "REPOMEMO_SETUP_CODE=/.test(ci)) problems.push(".github/workflows/ci.yml does not run scripts/smoke-test.mjs against the stack with a setup code");
  for (const step of ["/v1/setup/admin", "/v1/setup/complete", "/artifacts/upload", "/search", "/export", "/v1/workspaces/import", "/v1/system/backups"]) {
    if (!smoke.includes(step)) problems.push(`scripts/smoke-test.mjs does not use ${step}, a step of the first hour`);
  }
  for (const route of ["/v1/setup/admin", "/v1/setup/complete", "/v1/workspaces/{workspace_id}/artifacts/upload", "/v1/workspaces/{workspace_id}/search", "/v1/workspaces/{workspace_id}/export", "/v1/workspaces/import", "/v1/system/backups"]) {
    if (!readFileSync("apps/server/src/lib.rs", "utf8").includes(route)) problems.push(`the server has no ${route}, which the smoke test uses`);
  }
}

// Backups and restores in containers (V0.4.25): the volumes are copied out (the database and the files only with the stack
// stopped), restored into empty volumes only, never over an existing copy, and a CI job restores into a second stack.
{
  const backup = read("deploy/backup-volumes.sh");
  const restore = read("deploy/restore-volumes.sh");
  for (const short of ["repomemo-backups", "repomemo-data", "repomemo-db", "repomemo-files"]) {
    if (!backup.includes(short)) problems.push(`deploy/backup-volumes.sh does not know the ${short} volume`);
  }
  if (!/compose stop/.test(backup) || !/compose start/.test(backup)) problems.push("deploy/backup-volumes.sh does not stop the stack for the database and the files, and start it again");
  if (!/never overwritten/.test(backup) || !/backup\.txt/.test(backup)) problems.push("deploy/backup-volumes.sh can overwrite an earlier copy");
  if (!/is not empty; refusing/.test(restore) || !/a container uses/.test(restore)) problems.push("deploy/restore-volumes.sh does not refuse a volume that is filled or in use");
  if (/\brm -rf\b|volume rm/.test((backup + restore).replace(/#[^\n]*/g, "")) ) problems.push("the volume scripts delete something");
  for (const [file, text] of [["backup-volumes.sh", backup], ["restore-volumes.sh", restore], ["migrate-volumes.sh", read("deploy/migrate-volumes.sh")]]) {
    if (!/alpine:[\d.]+@sha256:[0-9a-f]{64}/.test(text)) problems.push(`deploy/${file} runs a helper image that is not pinned by digest`);
  }
  if (!/^  volume-restore:/m.test(ci) || !/restore-volumes\.sh --from/.test(ci)) problems.push(".github/workflows/ci.yml does not restore a copy into a second stack");
}

// The upgrade script (V0.4.24): a backup through the API first, a wait until healthy, the old version put back, and
// no volume ever deleted.
{
  const upgrade = read("deploy/upgrade.sh");
  if (!upgrade.includes("/v1/system/backups")) problems.push("deploy/upgrade.sh does not make its backup through the API");
  if (!/--wait\b/.test(upgrade) || !/wait_ready/.test(upgrade)) problems.push("deploy/upgrade.sh does not wait for the services to be healthy");
  if (!/set_version "\$old"/.test(upgrade)) problems.push("deploy/upgrade.sh does not put the old version back when the new one does not turn healthy");
  if (/\bdown\b[^\n]*(-v|--volumes)|volume rm|volume prune|rm -rf/.test(upgrade.replace(/#[^\n]*/g, ""))) problems.push("deploy/upgrade.sh can delete data; it must never remove a volume");
  if (upgrade.indexOf("make a backup") > upgrade.indexOf('set_version "$version"') || !upgrade.includes('echo "upgrade: making a backup"')) problems.push("deploy/upgrade.sh changes .env before the backup");
  if (!/^  upgrade-script:/m.test(ci) || !/upgrade\.sh --version broken/.test(ci)) problems.push(".github/workflows/ci.yml does not try the upgrade script with a broken tag");
}

// The scripts that make a .env (V0.4.23): shell and PowerShell, strong secrets, never an overwrite, and tried in CI.
{
  const sh = read("deploy/init.sh");
  const ps = read("deploy/init.ps1");
  for (const [name, text] of [["init.sh", sh], ["init.ps1", ps]]) {
    for (const needle of ["REPOMEMO_JWT_SECRET", "REPOMEMO_SECRET_KEY", "REPOMEMO_HOST", "COMPOSE_PROFILES"]) {
      if (!text.includes(needle)) problems.push(`deploy/${name} does not set ${needle}`);
    }
    if (!/exists already/.test(text)) problems.push(`deploy/${name} does not refuse to overwrite a .env`);
  }
  if (!/openssl rand -hex 32/.test(sh) || !/RandomNumberGenerator/.test(ps)) problems.push("the init scripts do not draw their secrets from a strong source");
  if (!/umask 077/.test(sh)) problems.push("deploy/init.sh does not keep the .env to its owner");
  if (!attributes.split(/\r?\n/).some((line) => line.startsWith("deploy/*.sh") && /eol=lf/.test(line))) problems.push(".gitattributes does not keep deploy/*.sh on LF line endings");
  if (!/^  init-scripts:/m.test(ci) || !/init\.sh/.test(ci) || !/init\.ps1/.test(ci)) problems.push(".github/workflows/ci.yml does not try both init scripts");
}

// The data is in three volumes of its own and a 0.3.x install can move into them (V0.4.22).
for (const [file, text] of [["docker-compose.yml", compose], ["deploy/api.yml", read("deploy/api.yml")]]) {
  for (const [volume, variable, folder] of [["repomemo-db", "REPOMEMO_DB_DIR", "/var/lib/repomemo-db"], ["repomemo-files", "REPOMEMO_BLOB_DIR", "/var/lib/repomemo-files"], ["repomemo-backups", "REPOMEMO_BACKUP_DIR", "/var/lib/repomemo-backups"]]) {
    if (!text.includes(`${variable}: ${folder}`)) problems.push(`${file} does not set ${variable} to ${folder}`);
    if (!text.includes(`- ${volume}:${folder}`)) problems.push(`${file} does not mount the ${volume} volume at ${folder}`);
    if (!new RegExp(`^volumes:[\\s\\S]*^  ${volume}:`, "m").test(text)) problems.push(`${file} does not declare the ${volume} volume`);
  }
}
{
  const migrate = read("deploy/migrate-volumes.sh");
  for (const volume of ["repomemo-data", "repomemo-db", "repomemo-files", "repomemo-backups"]) {
    if (!migrate.includes(volume)) problems.push(`deploy/migrate-volumes.sh does not know the ${volume} volume`);
  }
  if (/\brm -rf\b/.test(migrate.replace(/#[^\n]*/g, "").replace(/<<EOF[\s\S]*?\nEOF/, ""))) problems.push("deploy/migrate-volumes.sh deletes something; it must only copy");
  if (!/^  volume-migration:/m.test(ci) || !/migrate-volumes\.sh/.test(ci)) problems.push(".github/workflows/ci.yml does not try the move of a single volume into three");
}

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
if (!/image:\s*\$\{REPOMEMO_CONVERTER_IMAGE:-/.test(converterBlock ?? "") || !/image:\s*\$\{REPOMEMO_CONVERTER_IMAGE:-repomemo-converter:/.test(composeBuild)) problems.push("docker-compose.yml: the converter cannot start an image given by REPOMEMO_CONVERTER_IMAGE");
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
