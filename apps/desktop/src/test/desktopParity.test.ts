import { describe, expect, it } from "vitest";

/** The desktop app is the web client in a window (V0.4.43): one bundle and one entry, and every difference written down. */
const raw = (files: Record<string, string>): string => Object.values(files)[0];
const pages = import.meta.glob("../../*.html", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const viteConfig = raw(import.meta.glob("../../vite.config.ts", { query: "?raw", import: "default", eager: true }) as Record<string, string>);
const shellConfig = raw(import.meta.glob("../../src-tauri/tauri.conf.json", { query: "?raw", import: "default", eager: true }) as Record<string, string>);
const parity = raw(import.meta.glob("../../../../docs/DESKTOP_PARITY.md", { query: "?raw", import: "default", eager: true }) as Record<string, string>);

const sources = import.meta.glob("../**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const code = Object.entries(sources).filter(([path]) => path.startsWith("../") && !path.startsWith("../test/") && !path.startsWith("../generated/"));

const sourceOf = (path: string): string => sources[path];

describe("one bundle and one entry", () => {
  it("has a single page that loads a single script", () => {
    const html = pages["../../index.html"];
    expect([...html.matchAll(/<script\b[^>]*>/g)]).toHaveLength(1);
    expect(html).toContain('<script type="module" src="/src/main.tsx"></script>');
    expect(Object.keys(pages)).toEqual(["../../index.html"]);
  });

  it("builds one bundle, which the shell serves", () => {
    expect(viteConfig).not.toMatch(/\binput\s*:/);
    const shell = JSON.parse(shellConfig) as { build: { frontendDist: string }; app: { windows: Array<{ url?: string }> } };
    expect(shell.build.frontendDist).toBe("../dist");
    expect(shell.app.windows.every((window) => window.url === undefined)).toBe(true);
  });

  it("mounts the client in one place, which chooses between exactly two starts", () => {
    expect(code.filter(([, source]) => source.includes("createRoot(")).map(([path]) => path)).toEqual(["../main.tsx"]);
    const start = sourceOf("../App.tsx");
    expect([...start.matchAll(/<(\w+) \/>/g)].map((match) => match[1]).sort()).toEqual(["DesktopApp", "SharedWebApp"]);
    expect(sourceOf("../components/DesktopApp.tsx")).toContain("<SharedWebApp />");
  });
});

describe("every difference is written down", () => {
  const rows = parity
    .split("\n")
    .filter((line) => line.startsWith("| ") && !line.startsWith("| Area") && !line.startsWith("| ---"))
    .map((line) => line.split("|").slice(1, -1).map((cell) => cell.trim()));

  it("has a row for each of the six areas the audit names", () => {
    const areas = rows.map((row) => row[0].toLowerCase());
    for (const area of ["transport", "downloads", "links to other sites", "notifications", "shortcuts", "window"]) expect(areas, area).toContain(area);
  });

  it("says, for each row, the release that closes it or the reason it stays", () => {
    expect(rows.length).toBeGreaterThanOrEqual(10);
    for (const row of rows) {
      expect(row, row[0]).toHaveLength(4);
      expect(row[3], row[0]).toMatch(/^(Closed by V0\.4\.\d+|Stays: .{12,})/);
    }
  });

  it("lists exactly the files that branch on the desktop runtime or call the shell", () => {
    const listed = parity
      .slice(parity.indexOf("## Files that know about the desktop"))
      .split("\n")
      .flatMap((line) => /^- `(src\/[^`]+)`$/.exec(line)?.[1] ?? []);
    const knowing = code
      .filter(([, source]) => /isDesktopRuntime|__TAURI_INTERNALS__|@tauri-apps|desktopSession|installExternalLinkHandler/.test(source))
      .map(([path]) => `src/${path.slice(3)}`);
    expect([...listed].sort()).toEqual([...knowing].sort());
  });
});
