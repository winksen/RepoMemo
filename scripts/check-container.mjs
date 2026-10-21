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
const caddyfile = read("docker/Caddyfile");
const example = read(".env.example");
const ignore = read(".dockerignore");
const vite = read("apps/desktop/vite.config.ts");
const serverManifest = read("apps/server/Cargo.toml");

// No tabs in YAML, and files that Linux reads keep LF line endings in the repository.
if (/\t/.test(compose)) problems.push("docker-compose.yml contains a tab; YAML allows spaces only");
const attributes = read(".gitattributes");
for (const pattern of ["Dockerfile", "docker/Caddyfile", "docker/*.sh", "docker-compose.yml"]) {
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
for (const stage of ["web", "build", "server", "proxy"]) {
  if (!stages.includes(stage)) problems.push(`Dockerfile has no stage named ${stage}`);
}
for (const match of compose.matchAll(/^\s+target:\s*(\S+)/gm)) {
  if (!stages.includes(match[1])) problems.push(`docker-compose.yml builds the target ${match[1]}, which the Dockerfile does not have`);
}
for (const stage of ["server", "proxy"]) {
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

// The binary the Dockerfile copies is the one the server package builds.
const packageName = /^name\s*=\s*"([^"]+)"/m.exec(serverManifest)?.[1];
if (!packageName || !dockerfile.includes(`-p ${packageName}`) || !dockerfile.includes(`target/release/${packageName}`)) {
  problems.push(`the Dockerfile does not build and copy the server package (${packageName ?? "unknown"})`);
}
if (!/ENTRYPOINT \["repomemo-server"\]/.test(dockerfile)) problems.push("the server image does not start repomemo-server");
if (!/USER\s+repomemo/.test(dockerfile)) problems.push("the server image runs as root");
if (/^\s*(ENV|ARG)\s+\S*(SECRET|PASSWORD|TOKEN|JWT)\S*=/im.test(dockerfile)) problems.push("the Dockerfile bakes a secret into an image");

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
const composeOnly = new Set(["REPOMEMO_HOST", "REPOMEMO_VERSION"]);
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
else if (!caddyfile.includes(`reverse_proxy server:${listens}`)) problems.push(`docker/Caddyfile does not proxy to server:${listens}`);
if (listens && !new RegExp(`127\\.0\\.0\\.1:${listens}/health/ready`).test(dockerfile)) problems.push("the server image's health check does not ask the port the server listens on");
const proxyAddress = /proxy:[\s\S]*?ipv4_address:\s*(\S+)/.exec(compose)?.[1];
const trusted = /REPOMEMO_TRUSTED_PROXIES:\s*(\S+)/.exec(compose)?.[1];
if (!proxyAddress || proxyAddress !== trusted) problems.push(`the server must trust the proxy's address (proxy ${proxyAddress ?? "none"}, trusted ${trusted ?? "none"})`);
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
if (!caddyfile.includes("import /etc/caddy/csp.caddy") || !dockerfile.includes("/etc/caddy/csp.caddy")) problems.push("the Caddyfile and the Dockerfile disagree about the content security policy snippet");
if (!caddyfile.includes("flush_interval -1")) problems.push("the proxy buffers the live event streams");
if (!/\{\$REPOMEMO_HOST/.test(caddyfile)) problems.push("the Caddyfile does not use REPOMEMO_HOST");

// Nothing private goes into the build context.
for (const pattern of [".git", ".env", "workspace-data", "target"]) {
  if (!ignore.split(/\r?\n/).includes(pattern)) problems.push(`.dockerignore does not leave out ${pattern}`);
}

if (problems.length) {
  console.error(`The container files have ${problems.length} problem${problems.length === 1 ? "" : "s"}:`);
  for (const problem of problems) console.error(` - ${problem}`);
  process.exit(1);
}
console.log(`Container files agree (${stages.length} stages, ${used.size} variables, proxy ${proxyAddress} to server:${listens}, git ${serverGit} on Debian ${baseRelease}).`);
