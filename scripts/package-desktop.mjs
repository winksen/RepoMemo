#!/usr/bin/env node
// An administrator's command to hand a team a desktop app already set for their server (V0.4.48).
//
//   npm run desktop:package -- --server https://memo.example.com --name "Acme Memo"
//   npm run desktop:package -- --server https://memo.example.com --name "Acme Memo" --from ./ci-artifact
//
// It makes, in dist-desktop/: a portable folder as a zip (the executable, repomemo-desktop.json, a readme), the Windows installer
// (when one exists) and the same configuration file next to it for IT to place, and a SHA256SUMS over them.
//
// Without --from it builds the app (Rust, Node and the Tauri CLI are needed) with the address compiled in (V0.4.46), so even the
// executable alone talks to that server and no other. With --from FOLDER it builds nothing: it takes repomemo-desktop(.exe) and
// the *-setup.exe from a folder (the artifact CI uploads, V0.4.49) and adds the configuration file (V0.4.47), which is how an
// administrator without Rust installed does it. The configuration is checked by the rules the app checks it by.

import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { buildZip } from "./package-server.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const CONFIG_FILE = "repomemo-desktop.json";
const MAX_NAME_CHARS = 60;
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

// ---- the rules of the app (apps/desktop/src-tauri/src/connection.rs and config.rs), in the same words

const PRIVATE_V4 = [/^127\./, /^10\./, /^192\.168\./, /^172\.(1[6-9]|2\d|3[01])\./, /^169\.254\./];

function isLocalHost(hostname) {
  const host = hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (host.includes(":")) return host === "::1" || /^f[cd][0-9a-f]{2}:/.test(host) || /^fe[89ab][0-9a-f]:/.test(host);
  if (/^\d+\.\d+\.\d+\.\d+$/.test(host)) return PRIVATE_V4.some((pattern) => pattern.test(host));
  return host === "localhost" || !host.includes(".") || [".localhost", ".local", ".lan", ".internal", ".home.arpa"].some((suffix) => host.endsWith(suffix));
}

/** A server address as the app reads it: its origin, or an Error that says why not. */
export function normalizeServer(input) {
  const typed = String(input ?? "").trim();
  if (!typed) throw new Error("Enter the address of your RepoMemo server.");
  let url;
  try {
    url = new URL(typed.includes("://") ? typed : `https://${typed}`);
  } catch {
    throw new Error("That is not a web address.");
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") throw new Error("The address must start with https:// (or http:// on your own network).");
  if (!url.hostname) throw new Error("The address has no host name.");
  if (url.username || url.password) throw new Error("Leave the user name and password out of the address; you sign in on the next screen.");
  if (url.search || url.hash || (url.pathname !== "/" && url.pathname !== "")) throw new Error("Enter only the server's address, such as https://repomemo.example.com.");
  if (url.protocol === "http:" && !isLocalHost(url.hostname)) throw new Error("A server on the internet must use https://. Plain http:// is only for a server on your own network.");
  return url.origin;
}

/** The text of repomemo-desktop.json for these choices, checked the way the app checks a file. */
export function buildConfig({ server, name, support, locked = true }) {
  const config = { server: normalizeServer(server) };
  if (name !== undefined) {
    const folded = String(name).split(/\s+/).filter(Boolean).join(" ");
    // eslint-disable-next-line no-control-regex
    if (!folded || [...folded].length > MAX_NAME_CHARS || /[\u0000-\u001f\u007f]/.test(folded)) throw new Error(`name: must be one line of 1 to ${MAX_NAME_CHARS} characters`);
    config.name = folded;
  }
  config.locked = Boolean(locked);
  if (support !== undefined) {
    let url;
    try {
      url = new URL(String(support).trim());
    } catch {
      throw new Error("support: must be an http:// or https:// address without a login");
    }
    if (!["http:", "https:"].includes(url.protocol) || !url.hostname || url.username || url.password) throw new Error("support: must be an http:// or https:// address without a login");
    config.support = url.toString();
  }
  return `${JSON.stringify(config, null, 2)}\n`;
}

/** A name for a file: letters and digits, one dash between words. */
export function slug(name) {
  return String(name ?? "").normalize("NFKD").replace(/[̀-ͯ]/g, "").replace(/[^A-Za-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "RepoMemo";
}

function handoutReadme({ name, server, version, hasInstaller }) {
  return `${name}: RepoMemo desktop app for ${server}
Version ${version}

To use it
  1. Unpack this folder anywhere you can write to, and start RepoMemo.exe.
  2. The app opens on ${server}. Sign in with the account you were given.
  repomemo-desktop.json beside the program is what points it there. Keep the two together; move both or neither.

${hasInstaller ? `For IT: the installer (RepoMemo-setup.exe) installs the program for one person without administrator rights. Place a copy of
repomemo-desktop.json in %APPDATA%\\com.repomemo.desktop\\ (or beside the installed program) for the same result.
` : ""}Nothing about your account is stored in these files. The session is kept in the operating system's credential store.
`;
}

/** The zip of the portable folder. */
export function portableZip({ exeBytes, configText, readmeText, exeName = "RepoMemo.exe", mtime = 0 }) {
  const top = "RepoMemo";
  const entries = [
    { path: `${top}/${exeName}`, bytes: exeBytes, mode: 0o755 },
    { path: `${top}/${CONFIG_FILE}`, bytes: Buffer.from(configText), mode: 0o644 },
    { path: `${top}/README.txt`, bytes: Buffer.from(readmeText), mode: 0o644 },
  ];
  return buildZip(entries, mtime);
}

function findExecutables(folder) {
  const names = readdirSync(folder);
  const exe = names.find((name) => /^repomemo-desktop(\.exe)?$/i.test(name));
  const installer = names.find((name) => /-setup\.exe$/i.test(name));
  return { exe: exe && join(folder, exe), installer: installer && join(folder, installer) };
}

function buildWithTauri(server) {
  const run = (command, args, cwd) => {
    const result = spawnSync(command, args, { cwd, stdio: "inherit", shell: process.platform === "win32", env: { ...process.env, REPOMEMO_PRESET_SERVER: server } });
    if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed (see above); is Rust installed? Or use --from with the app CI builds`);
  };
  run("npx", ["tauri", "build", "--bundles", "nsis"], join(root, "apps", "desktop"));
  return { exe: join(root, "target", "release", process.platform === "win32" ? "repomemo-desktop.exe" : "repomemo-desktop"), installerFolder: join(root, "target", "release", "bundle", "nsis") };
}

/** Makes the handout; returns the files written. */
export function packageDesktop({ server, name, support, locked = true, from, out, version, mtime = 0, build = buildWithTauri }) {
  const configText = buildConfig({ server, name, support, locked });
  const origin = JSON.parse(configText).server;
  let found;
  if (from) {
    if (!existsSync(from)) throw new Error(`${from} does not exist`);
    found = findExecutables(from);
  } else {
    const built = build(origin);
    found = { exe: built.exe, installer: existsSync(built.installerFolder) ? findExecutables(built.installerFolder).installer : undefined };
  }
  if (!found.exe || !existsSync(found.exe)) throw new Error(`no repomemo-desktop executable in ${from ?? "the build"}; build the app or point --from at the folder CI uploads`);
  const label = name ?? "RepoMemo";
  const stem = `RepoMemo-${slug(label)}`;
  mkdirSync(out, { recursive: true });
  const written = [];
  const put = (file, bytes) => {
    writeFileSync(join(out, file), bytes);
    written.push(file);
  };
  put(`${stem}-portable.zip`, portableZip({ exeBytes: readFileSync(found.exe), configText, readmeText: handoutReadme({ name: label, server: origin, version, hasInstaller: Boolean(found.installer) }), mtime }));
  if (found.installer) {
    copyFileSync(found.installer, join(out, `${stem}-setup.exe`));
    written.push(`${stem}-setup.exe`);
  }
  put(CONFIG_FILE, configText);
  const sums = written.map((file) => `${sha256(readFileSync(join(out, file)))}  ${file}`).sort((a, b) => a.slice(66).localeCompare(b.slice(66)));
  put("SHA256SUMS", `${sums.join("\n")}\n`);
  return written;
}

function arg(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at > 0 && process.argv[at + 1] ? process.argv[at + 1] : fallback;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    if (!arg("server")) throw new Error('give the server: npm run desktop:package -- --server https://memo.example.com --name "Acme Memo"');
    const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
    const out = arg("out", join(root, "dist-desktop"));
    const written = packageDesktop({
      server: arg("server"),
      name: arg("name"),
      support: arg("support"),
      locked: !process.argv.includes("--open"),
      from: arg("from"),
      out,
      version,
      mtime: Number(process.env.SOURCE_DATE_EPOCH ?? 0),
    });
    console.log(`desktop: ${written.join(", ")} in ${out}`);
  } catch (error) {
    console.error(`package-desktop: ${error.message}`);
    process.exit(1);
  }
}
