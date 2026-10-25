#!/usr/bin/env node
// Packs the server for a direct install (V0.4.31): the binary, the built web app, the settings example, the service files,
// a readme and checksums, in the same layout on every system, as a .tar.gz (Linux) or a .zip (Windows). No archive tool
// is needed: both formats are written here, in a fixed order and with a fixed time, so the same input is the same archive.
//
//   node scripts/package-server.mjs --binary target/release/repomemo-server --web apps/desktop/dist \
//        --os linux --arch x64 --out dist-server
//
// Options: --version X (default: package.json), --service-dir DIR (default: deploy/direct/service when it exists).
// SOURCE_DATE_EPOCH sets the time recorded in the archive (default 0).
// It writes <out>/repomemo-server-<version>-<os>-<arch>.tar.gz|zip and adds that archive's line to <out>/SHA256SUMS.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { crc32, deflateRawSync, gzipSync } from "node:zlib";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const OSES = { linux: { extension: "tar.gz", binary: "repomemo-server" }, windows: { extension: "zip", binary: "repomemo-server.exe" } };
const ARCHES = ["x64", "arm64"];

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

/** Every file under `folder`, sorted, as [path relative to it with forward slashes, bytes]. */
function filesUnder(folder) {
  const found = [];
  const walk = (current) => {
    for (const name of readdirSync(current).sort()) {
      const path = join(current, name);
      if (statSync(path).isDirectory()) walk(path);
      else found.push([relative(folder, path).split("\\").join("/"), readFileSync(path)]);
    }
  };
  walk(folder);
  return found;
}

/** The entries of the archive: { path, bytes, mode }, in a fixed order. */
export function layout({ top, os, binary, web, envExample, serviceFiles, version, readme }) {
  const entries = [];
  const add = (path, bytes, mode = 0o644) => entries.push({ path: `${top}/${path}`, bytes: Buffer.isBuffer(bytes) ? bytes : Buffer.from(bytes), mode });
  add(`bin/${OSES[os].binary}`, binary, 0o755);
  for (const [path, bytes] of web) add(`web/${path}`, bytes);
  add("config/repomemo.env.example", envExample);
  for (const [path, bytes] of serviceFiles) add(`service/${path}`, bytes, path.endsWith(".sh") ? 0o755 : 0o644);
  add("README.txt", readme);
  add("VERSION", `${version}\n`);
  // The checksums cover everything above, and are the last file.
  const sums = entries.map((entry) => `${sha256(entry.bytes)}  ${entry.path.slice(top.length + 1)}`).join("\n");
  add("SHA256SUMS", `${sums}\n`);
  return entries;
}

// ---- tar (ustar), gzipped

function octal(value, width) {
  return value.toString(8).padStart(width - 1, "0") + "\0";
}

function tarHeader(path, size, mode, mtime, type = "0") {
  let name = path;
  let prefix = "";
  if (Buffer.byteLength(name) > 100) {
    const cut = name.lastIndexOf("/", name.length - 1);
    const probe = name.slice(0, cut);
    if (cut < 0 || Buffer.byteLength(name.slice(cut + 1)) > 100 || Buffer.byteLength(probe) > 155) throw new Error(`the path is too long for a tar header: ${path}`);
    prefix = probe;
    name = name.slice(cut + 1);
  }
  const header = Buffer.alloc(512);
  header.write(name, 0, 100, "utf8");
  header.write(octal(mode, 8), 100);
  header.write(octal(0, 8), 108);
  header.write(octal(0, 8), 116);
  header.write(octal(size, 12), 124);
  header.write(octal(mtime, 12), 136);
  header.write("        ", 148);
  header.write(type, 156);
  header.write("ustar\0", 257);
  header.write("00", 263);
  header.write("root", 265);
  header.write("root", 297);
  header.write(prefix, 345, 155, "utf8");
  const checksum = header.reduce((total, byte) => total + byte, 0);
  header.write(`${checksum.toString(8).padStart(6, "0")}\0 `, 148);
  return header;
}

export function buildTarGz(entries, mtime = 0) {
  const parts = [];
  const directories = new Set();
  for (const entry of entries) {
    const segments = entry.path.split("/").slice(0, -1);
    for (let index = 1; index <= segments.length; index += 1) directories.add(segments.slice(0, index).join("/"));
  }
  for (const directory of [...directories].sort()) parts.push(tarHeader(`${directory}/`, 0, 0o755, mtime, "5"));
  for (const entry of entries) {
    parts.push(tarHeader(entry.path, entry.bytes.length, entry.mode, mtime), entry.bytes, Buffer.alloc((512 - (entry.bytes.length % 512)) % 512));
  }
  parts.push(Buffer.alloc(1024));
  // A fixed gzip header (no name, no time) keeps the archive the same from one run to the next.
  return gzipSync(Buffer.concat(parts), { level: 9 });
}

// ---- zip

function dosDateTime(seconds) {
  const date = new Date(Math.max(seconds, 315532800) * 1000);
  return {
    time: (date.getUTCHours() << 11) | (date.getUTCMinutes() << 5) | (date.getUTCSeconds() >> 1),
    day: ((date.getUTCFullYear() - 1980) << 9) | ((date.getUTCMonth() + 1) << 5) | date.getUTCDate(),
  };
}

export function buildZip(entries, mtime = 0) {
  const { time, day } = dosDateTime(mtime);
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const entry of entries) {
    const name = Buffer.from(entry.path, "utf8");
    const data = deflateRawSync(entry.bytes, { level: 9 });
    const sum = crc32(entry.bytes);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0x0800, 6); // UTF-8 names
    local.writeUInt16LE(8, 8);
    local.writeUInt16LE(time, 10);
    local.writeUInt16LE(day, 12);
    local.writeUInt32LE(sum, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(entry.bytes.length, 22);
    local.writeUInt16LE(name.length, 26);
    locals.push(local, name, data);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE((3 << 8) | 20, 4); // made on Unix, so the mode is kept
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(8, 10);
    central.writeUInt16LE(time, 12);
    central.writeUInt16LE(day, 14);
    central.writeUInt32LE(sum, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(entry.bytes.length, 24);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt32LE(((0o100000 | entry.mode) << 16) >>> 0, 38);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, name);
    offset += 30 + name.length + data.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

// ---- the command

function readme({ version, os, arch }) {
  const run = os === "windows" ? "bin\\repomemo-server.exe" : "bin/repomemo-server";
  return `RepoMemo ${version} for ${os} (${arch}): the server, the web app and what a direct install needs.

What is here
  bin/        the server (${basename(run)})
  web/        the built web app; REPOMEMO_WEB_DIR=<this folder>/web makes the server serve it on its own port
  config/     repomemo.env.example: the settings of a direct install, with the secrets to fill in
  service/    files that run the server as a service (systemd on Linux, a Windows service)
  SHA256SUMS  the checksum of every file here: check them with   sha256sum -c SHA256SUMS
  VERSION     the release

Try it without installing anything
  1. copy config/repomemo.env.example to repomemo.env and fill in REPOMEMO_JWT_SECRET and REPOMEMO_SECRET_KEY
     (32 or more random characters each), and set REPOMEMO_WEB_DIR to the full path of the web folder
  2. load that file into the environment and start   ${run}
  3. open the address in REPOMEMO_SERVER_ADDR (http://127.0.0.1:3020) and finish the first-run setup
  The data is written to workspace-data/ under the folder you start it from.

For a server that stays up, use the service files and the guide: mindmap/technical/operations.md in the source tree,
or https://github.com/repomemo/repomemo. Behind a front door (TLS), set REPOMEMO_PUBLIC_URL and REPOMEMO_TRUST_PROXY.
`;
}

function arg(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at > 0 && process.argv[at + 1] ? process.argv[at + 1] : fallback;
}

export function packageServer({ binaryPath, webPath, os, arch, out, version, serviceDir, mtime }) {
  if (!OSES[os]) throw new Error(`--os must be linux or windows, not ${os}`);
  if (!ARCHES.includes(arch)) throw new Error(`--arch must be x64 or arm64, not ${arch}`);
  if (!existsSync(binaryPath)) throw new Error(`the server binary ${binaryPath} does not exist; build it first (cargo build --release -p repomemo-server)`);
  if (!existsSync(join(webPath, "index.html"))) throw new Error(`${webPath} has no index.html; build the web app first (npm run build)`);
  const envFile = join(root, "deploy", "direct", "repomemo.env.example");
  const top = `repomemo-server-${version}-${os}-${arch}`;
  const entries = layout({
    top,
    os,
    binary: readFileSync(binaryPath),
    web: filesUnder(webPath),
    envExample: readFileSync(envFile),
    serviceFiles: serviceDir && existsSync(serviceDir) ? filesUnder(serviceDir) : [],
    version,
    readme: readme({ version, os, arch }),
  });
  const archive = OSES[os].extension === "zip" ? buildZip(entries, mtime) : buildTarGz(entries, mtime);
  mkdirSync(out, { recursive: true });
  const file = `${top}.${OSES[os].extension}`;
  writeFileSync(join(out, file), archive);
  const sumsPath = join(out, "SHA256SUMS");
  const kept = existsSync(sumsPath) ? readFileSync(sumsPath, "utf8").split("\n").filter((line) => line && !line.endsWith(`  ${file}`)) : [];
  writeFileSync(sumsPath, `${[...kept, `${sha256(archive)}  ${file}`].sort((a, b) => a.slice(66).localeCompare(b.slice(66))).join("\n")}\n`);
  return { file, entries: entries.length, bytes: archive.length };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const version = arg("version", JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version);
    const serviceDefault = join(root, "deploy", "direct", "service");
    const result = packageServer({
      binaryPath: arg("binary", ""),
      webPath: arg("web", join(root, "apps", "desktop", "dist")),
      os: arg("os", process.platform === "win32" ? "windows" : "linux"),
      arch: arg("arch", process.arch === "arm64" ? "arm64" : "x64"),
      out: arg("out", join(root, "dist-server")),
      version,
      serviceDir: arg("service-dir", serviceDefault),
      mtime: Number(process.env.SOURCE_DATE_EPOCH ?? 0),
    });
    console.log(`${result.file}: ${result.entries} files, ${result.bytes} bytes`);
  } catch (error) {
    console.error(`package-server: ${error.message}`);
    process.exit(1);
  }
}
