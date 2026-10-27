import { readFileSync } from "node:fs";
import { loadEnv, type Plugin } from "vite";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

/**
 * The content security policy of the built web app (review M2): scripts only
 * from the app's own files, requests only to itself and the API, images from
 * itself, `blob:` and `data:`, frames only for `blob:` document previews, and
 * no plugins or base-URL tricks.
 */
function contentSecurityPolicy(apiUrl: string): { meta: string; header: string } {
  let apiOrigin = "";
  try {
    apiOrigin = new URL(apiUrl).origin;
  } catch {
    // An unparsable address leaves connect-src at 'self'.
  }
  const directives = [
    "default-src 'self'",
    "script-src 'self'",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' blob: data:",
    "font-src 'self' data:",
    `connect-src 'self'${apiOrigin ? ` ${apiOrigin}` : ""}`,
    "frame-src blob:",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'self'",
  ];
  const meta = directives.join("; ");
  // A meta tag cannot carry frame-ancestors; the host sends it as a header.
  return { meta, header: `${meta}; frame-ancestors 'none'` };
}

/**
 * The `config.json` the host serves beside the built page (V0.4.14), with the address the app was built for:
 * a host that sets its own (the web image, the proxy of the compose file) replaces it, so one build serves any server.
 */
function runtimeConfigPlugin(apiUrl: string): Plugin {
  return {
    name: "repomemo-runtime-config",
    apply: "build",
    generateBundle() {
      this.emitFile({ type: "asset", fileName: "config.json", source: `${JSON.stringify({ apiUrl }, null, 2)}\n` });
    },
  };
}

/** Puts the policy in the built page and writes the header value for the host. */
function contentSecurityPolicyPlugin(apiUrl: string): Plugin {
  const policy = contentSecurityPolicy(apiUrl);
  return {
    name: "repomemo-content-security-policy",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler: (html) => html.replace("<head>", `<head>\n    <meta http-equiv="Content-Security-Policy" content="${policy.meta}" />`),
    },
    generateBundle() {
      this.emitFile({ type: "asset", fileName: "content-security-policy.txt", source: `${policy.header}\n` });
    },
  };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "VITE_");
  // An empty address means the API is on the page's own host (the container image); only an unset one falls back.
  const apiUrl = env.VITE_REPOMEMO_API_URL ?? "http://127.0.0.1:3020";
  return {
    plugins: [react(), contentSecurityPolicyPlugin(apiUrl), runtimeConfigPlugin(apiUrl)],
    // The release this build is, compared with the server's at start (V0.4.21).
    define: {
      __REPOMEMO_VERSION__: JSON.stringify((JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")) as { version: string }).version),
    },
    clearScreen: false,
    server: {
      strictPort: true,
      port: 3021,
    },
    // Only these variables are compiled into the bundle. Never the bare
    // `TAURI_` prefix: it would also inline TAURI_SIGNING_PRIVATE_KEY (and its
    // password) during a release build, shipping the updater signing key.
    envPrefix: ["VITE_", "TAURI_ENV_"],
    test: {
      environment: "jsdom",
      setupFiles: ["src/test/setup.ts"],
      include: ["src/**/*.test.{ts,tsx}"],
      // Vitest 4 restores only `vi.spyOn` spies; the reset also clears the calls and implementations of `vi.fn()` mocks, as each test expects.
      restoreMocks: true,
      mockReset: true,
    },
  };
});
