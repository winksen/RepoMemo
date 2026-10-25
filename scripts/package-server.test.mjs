// node --test scripts/package-server.test.mjs  (npm run package:test)
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { crc32, gunzipSync, inflateRawSync } from "node:zlib";
import { packageServer } from "./package-server.mjs";

function workspace() {
  const dir = mkdtempSync(join(tmpdir(), "repomemo-package-"));
  const web = join(dir, "web");
  mkdirSync(join(web, "assets"), { recursive: true });
  writeFileSync(join(web, "index.html"), '<div id="root"></div>');
  writeFileSync(join(web, "assets", "app-abc123.js"), "console.log(1)");
  const binary = join(dir, "repomemo-server");
  writeFileSync(binary, "BINARY");
  return { dir, web, binary, out: join(dir, "out") };
}

const run = (w, os, arch) => packageServer({ binaryPath: w.binary, webPath: w.web, os, arch, out: w.out, version: "9.9.9", serviceDir: null, mtime: 0 });

/** The files of a tar: { path, mode, bytes }. */
function readTar(archive) {
  const data = gunzipSync(archive);
  const files = [];
  for (let at = 0; at + 512 <= data.length; ) {
    const header = data.subarray(at, at + 512);
    if (header.every((byte) => byte === 0)) break;
    const text = (from, length) => header.toString("utf8", from, from + length).replace(/\0.*$/s, "");
    const size = parseInt(text(124, 12), 8);
    const prefix = text(345, 155);
    const name = prefix ? `${prefix}/${text(0, 100)}` : text(0, 100);
    let sum = 0;
    for (let index = 0; index < 512; index += 1) sum += index >= 148 && index < 156 ? 32 : header[index];
    assert.equal(parseInt(text(148, 8), 8), sum, `the checksum of the header of ${name}`);
    if (header[156] === 0x30) files.push({ path: name, mode: parseInt(text(100, 8), 8), bytes: data.subarray(at + 512, at + 512 + size) });
    at += 512 + Math.ceil(size / 512) * 512;
  }
  return files;
}

/** The files of a zip, read from its central directory: { path, mode, bytes }. */
function readZip(archive) {
  const end = archive.length - 22;
  assert.equal(archive.readUInt32LE(end), 0x06054b50);
  const count = archive.readUInt16LE(end + 10);
  let at = archive.readUInt32LE(end + 16);
  const files = [];
  for (let index = 0; index < count; index += 1) {
    assert.equal(archive.readUInt32LE(at), 0x02014b50);
    const compressed = archive.readUInt32LE(at + 20);
    const size = archive.readUInt32LE(at + 24);
    const nameLength = archive.readUInt16LE(at + 28);
    const mode = archive.readUInt32LE(at + 38) >>> 16;
    const local = archive.readUInt32LE(at + 42);
    const path = archive.toString("utf8", at + 46, at + 46 + nameLength);
    const start = local + 30 + archive.readUInt16LE(local + 26) + archive.readUInt16LE(local + 28);
    const bytes = inflateRawSync(archive.subarray(start, start + compressed));
    assert.equal(bytes.length, size);
    assert.equal(crc32(bytes), archive.readUInt32LE(at + 16), `the crc of ${path}`);
    files.push({ path, mode: mode & 0o777, bytes });
    at += 46 + nameLength;
  }
  return files;
}

test("a Linux archive has the same layout everywhere, with the binary executable", () => {
  const w = workspace();
  const result = run(w, "linux", "x64");
  assert.equal(result.file, "repomemo-server-9.9.9-linux-x64.tar.gz");
  const files = readTar(readFileSync(join(w.out, result.file)));
  const paths = files.map((file) => file.path);
  const top = "repomemo-server-9.9.9-linux-x64";
  for (const expected of ["bin/repomemo-server", "web/index.html", "web/assets/app-abc123.js", "config/repomemo.env.example", "README.txt", "VERSION", "SHA256SUMS"]) {
    assert.ok(paths.includes(`${top}/${expected}`), `${expected} is in the archive`);
  }
  assert.equal(files.find((file) => file.path.endsWith("bin/repomemo-server")).mode, 0o755);
  assert.equal(files.find((file) => file.path.endsWith("VERSION")).bytes.toString(), "9.9.9\n");
});

test("a Windows archive is a zip with repomemo-server.exe in the same layout", () => {
  const w = workspace();
  const result = run(w, "windows", "arm64");
  assert.equal(result.file, "repomemo-server-9.9.9-windows-arm64.zip");
  const files = readZip(readFileSync(join(w.out, result.file)));
  const top = "repomemo-server-9.9.9-windows-arm64";
  assert.ok(files.some((file) => file.path === `${top}/bin/repomemo-server.exe`));
  assert.ok(files.some((file) => file.path === `${top}/web/assets/app-abc123.js`));
  assert.equal(files.find((file) => file.path.endsWith(".exe")).mode, 0o755);
});

test("SHA256SUMS inside the archive covers every other file and is correct", () => {
  const w = workspace();
  const result = run(w, "linux", "arm64");
  const files = readTar(readFileSync(join(w.out, result.file)));
  const sums = files.find((file) => file.path.endsWith("/SHA256SUMS")).bytes.toString().trim().split("\n");
  const others = files.filter((file) => !file.path.endsWith("/SHA256SUMS"));
  assert.equal(sums.length, others.length);
  for (const line of sums) {
    const [hash, path] = line.split("  ");
    const file = others.find((candidate) => candidate.path.endsWith(`/${path}`));
    assert.ok(file, `${path} is in the archive`);
    assert.equal(createHash("sha256").update(file.bytes).digest("hex"), hash, `the checksum of ${path}`);
  }
});

test("the same input makes the same archive, and the outer SHA256SUMS names each archive once, sorted", () => {
  const w = workspace();
  const first = run(w, "linux", "x64");
  const bytes = readFileSync(join(w.out, first.file));
  run(w, "linux", "x64");
  assert.deepEqual(readFileSync(join(w.out, first.file)), bytes, "a second run changes nothing");
  run(w, "windows", "x64");
  const lines = readFileSync(join(w.out, "SHA256SUMS"), "utf8").trim().split("\n");
  assert.equal(lines.length, 2);
  assert.deepEqual(lines.map((line) => line.split("  ")[1]), ["repomemo-server-9.9.9-linux-x64.tar.gz", "repomemo-server-9.9.9-windows-x64.zip"]);
  assert.equal(lines[0].split("  ")[0], createHash("sha256").update(bytes).digest("hex"));
});

test("it refuses what cannot be packed, and says what to do", () => {
  const w = workspace();
  assert.throws(() => packageServer({ binaryPath: join(w.dir, "missing"), webPath: w.web, os: "linux", arch: "x64", out: w.out, version: "1.0.0", mtime: 0 }), /build it first/);
  assert.throws(() => packageServer({ binaryPath: w.binary, webPath: w.dir, os: "linux", arch: "x64", out: w.out, version: "1.0.0", mtime: 0 }), /npm run build/);
  assert.throws(() => run(w, "plan9", "x64"), /linux or windows/);
  assert.throws(() => run(w, "linux", "mips"), /x64 or arm64/);
});
