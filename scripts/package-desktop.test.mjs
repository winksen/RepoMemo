// node --test scripts/package-desktop.test.mjs  (npm run desktop:test)
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { buildConfig, normalizeServer, packageDesktop, slug } from "./package-desktop.mjs";

const example = new URL("../deploy/desktop/repomemo-desktop.example.json", import.meta.url);

test("the configuration for the documented choices is the sample administrators are given", () => {
  const text = buildConfig({ server: "memo.example.com", name: "Acme Memo", support: "https://intranet.example.com/help/memo" });
  assert.deepEqual(JSON.parse(text), JSON.parse(readFileSync(example, "utf8")));
  assert.ok(text.endsWith("}\n"));
});

test("the server is checked by the rules the app checks it by", () => {
  assert.equal(normalizeServer(" Memo.Example.com:443/ "), "https://memo.example.com");
  for (const local of ["http://localhost:3020", "http://192.168.1.20", "http://repomemo", "http://nas.local", "http://[::1]:3020"]) assert.ok(normalizeServer(local), local);
  for (const bad of ["", "   ", "ftp://example.com", "file:///etc/passwd", "javascript:alert(1)", "https://user:secret@example.com", "https://example.com/app", "https://example.com/?a=1", "http://memo.example.com", "http://8.8.8.8"]) {
    assert.throws(() => normalizeServer(bad), Error, bad);
  }
});

test("a name is one short line, a help link is a web address, and what is wrong stops the command", () => {
  assert.equal(JSON.parse(buildConfig({ server: "memo.example.com", name: "  Acme   Memo\n" })).name, "Acme Memo");
  assert.throws(() => buildConfig({ server: "memo.example.com", name: "" }), /name:/);
  assert.throws(() => buildConfig({ server: "memo.example.com", name: "n".repeat(61) }), /name:/);
  assert.throws(() => buildConfig({ server: "memo.example.com", name: "bell\u0007" }), /name:/);
  for (const bad of ["javascript:alert(1)", "https://u:p@x.example.com", "help"]) assert.throws(() => buildConfig({ server: "memo.example.com", support: bad }), /support:/, bad);
  assert.equal(JSON.parse(buildConfig({ server: "memo.example.com", locked: false })).locked, false);
  assert.throws(() => buildConfig({ server: "" }), /Enter the address/);
});

test("a file name is made of letters and digits", () => {
  assert.equal(slug("Acme Memo!"), "Acme-Memo");
  assert.equal(slug("Équipe Données"), "Equipe-Donnees");
  assert.equal(slug("!!!"), "RepoMemo");
});

function artifact(withInstaller) {
  const dir = mkdtempSync(join(tmpdir(), "repomemo-desktop-"));
  const from = join(dir, "ci");
  mkdirSync(from);
  writeFileSync(join(from, "repomemo-desktop.exe"), "EXE-BYTES");
  if (withInstaller) writeFileSync(join(from, "RepoMemo_0.4.48_x64-setup.exe"), "SETUP-BYTES");
  return { from, out: join(dir, "out") };
}

test("without Rust: the CI artifact plus the configuration file make a portable folder, the installer and the sums", () => {
  const { from, out } = artifact(true);
  const written = packageDesktop({ server: "https://memo.example.com", name: "Acme Memo", from, out, version: "9.9.9" });
  assert.deepEqual([...written].sort(), ["RepoMemo-Acme-Memo-portable.zip", "RepoMemo-Acme-Memo-setup.exe", "SHA256SUMS", "repomemo-desktop.json"]);
  const zip = readFileSync(join(out, "RepoMemo-Acme-Memo-portable.zip"));
  for (const name of ["RepoMemo/RepoMemo.exe", "RepoMemo/repomemo-desktop.json", "RepoMemo/README.txt"]) assert.ok(zip.includes(Buffer.from(name)), name);
  assert.equal(readFileSync(join(out, "RepoMemo-Acme-Memo-setup.exe"), "utf8"), "SETUP-BYTES");
  assert.equal(JSON.parse(readFileSync(join(out, "repomemo-desktop.json"), "utf8")).server, "https://memo.example.com");
  const sums = readFileSync(join(out, "SHA256SUMS"), "utf8");
  assert.equal(sums.trim().split("\n").length, 3, "the zip, the installer and the file; the sums do not list themselves");
});

test("the portable zip is the same bytes every time", () => {
  const { from, out } = artifact(false);
  packageDesktop({ server: "memo.example.com", name: "Acme Memo", from, out, version: "9.9.9" });
  const first = readFileSync(join(out, "RepoMemo-Acme-Memo-portable.zip"));
  packageDesktop({ server: "memo.example.com", name: "Acme Memo", from, out, version: "9.9.9" });
  assert.deepEqual(readFileSync(join(out, "RepoMemo-Acme-Memo-portable.zip")), first);
});

test("it stops, writing nothing, for a bad server or a folder with no app", () => {
  const { from, out } = artifact(false);
  assert.throws(() => packageDesktop({ server: "http://memo.example.com", from, out, version: "9.9.9" }), /https/);
  assert.throws(() => packageDesktop({ server: "memo.example.com", from: join(from, "nowhere"), out, version: "9.9.9" }), /does not exist/);
  const empty = mkdtempSync(join(tmpdir(), "repomemo-desktop-empty-"));
  assert.throws(() => packageDesktop({ server: "memo.example.com", from: empty, out, version: "9.9.9" }), /no repomemo-desktop executable/);
  assert.throws(() => readFileSync(join(out, "SHA256SUMS")));
});

const sampleCertificate = new URL("../apps/desktop/src-tauri/testdata/test-ca.pem", import.meta.url);

test("a certificate bundle for a private authority is checked, put beside the configuration file and named in it", () => {
  const { from, out } = artifact(true);
  const written = packageDesktop({ server: "memo.example.com", name: "Acme Memo", certificatePath: sampleCertificate.pathname.replace(/^\/([A-Za-z]:)/, "$1"), from, out, version: "9.9.9" });
  assert.ok(written.includes("repomemo-ca.pem"));
  assert.equal(JSON.parse(readFileSync(join(out, "repomemo-desktop.json"), "utf8")).certificate, "repomemo-ca.pem");
  assert.equal(readFileSync(join(out, "repomemo-ca.pem"), "utf8"), readFileSync(sampleCertificate, "utf8"));
  const zip = readFileSync(join(out, "RepoMemo-Acme-Memo-portable.zip"));
  assert.ok(zip.includes(Buffer.from("RepoMemo/repomemo-ca.pem")));
  assert.equal(readFileSync(join(out, "SHA256SUMS"), "utf8").trim().split("\n").length, 4);
});

test("a bundle that is not one, or holds a key, stops the command with nothing written", () => {
  const { from, out } = artifact(false);
  const dir = mkdtempSync(join(tmpdir(), "repomemo-ca-"));
  const text = join(dir, "text.pem");
  writeFileSync(text, "just some words");
  const withKey = join(dir, "key.pem");
  writeFileSync(withKey, `${readFileSync(sampleCertificate, "utf8")}\n-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n`);
  const huge = join(dir, "huge.pem");
  writeFileSync(huge, "x".repeat(300_000));
  assert.throws(() => packageDesktop({ server: "memo.example.com", certificatePath: text, from, out, version: "9.9.9" }), /holds no certificate/);
  assert.throws(() => packageDesktop({ server: "memo.example.com", certificatePath: withKey, from, out, version: "9.9.9" }), /private key/);
  assert.throws(() => packageDesktop({ server: "memo.example.com", certificatePath: huge, from, out, version: "9.9.9" }), /bytes/);
  assert.throws(() => packageDesktop({ server: "memo.example.com", certificatePath: join(dir, "missing.pem"), from, out, version: "9.9.9" }), /does not exist/);
  assert.throws(() => readFileSync(join(out, "SHA256SUMS")));
});
