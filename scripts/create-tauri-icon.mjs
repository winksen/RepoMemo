// Makes the desktop app's icon set from the logo (V0.4.51): every size Windows, macOS, Linux and the installer use, from
// apps/desktop/public/RM-logo.svg, written to apps/desktop/src-tauri/icons. The icons are committed; run this again only when the
// logo changes (`npm run icons`). Tauri's own CLI does the drawing; the phone folders it also writes are not used and are removed.
import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { resolve } from "node:path";

const logo = resolve("apps/desktop/public/RM-logo.svg");
const out = resolve("apps/desktop/src-tauri/icons");
const run = spawnSync("npx", ["tauri", "icon", logo, "--output", out], { stdio: "inherit", shell: process.platform === "win32" });
if (run.status !== 0) {
  console.error("icons: tauri icon failed; run npm install first");
  process.exit(1);
}
for (const folder of ["android", "ios"]) rmSync(resolve(out, folder), { recursive: true, force: true });
console.log(`icons: written to ${out}`);
