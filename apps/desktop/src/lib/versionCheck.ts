/**
 * Whether this client and the server it reached can work together (V0.4.21). Before 1.0 a new minor release may change
 * the API, so the major and minor numbers must agree; the patch number never matters. A server that does not say
 * its version (earlier than 0.4.3) is taken to be older.
 */
declare const __REPOMEMO_VERSION__: string;

/** The version this client was built as. */
export const CLIENT_VERSION: string = typeof __REPOMEMO_VERSION__ === "string" ? __REPOMEMO_VERSION__ : "0.0.0";

export type VersionVerdict =
  | { ok: true }
  | { ok: false; reason: "older" | "newer"; clientVersion: string; serverVersion: string | null };

/** The major and minor numbers of `1.2.3`, or `null` for something that is not a version. */
function majorMinor(version: string | undefined | null): [number, number] | null {
  const match = /^(\d+)\.(\d+)\.\d+/.exec((version ?? "").trim());
  return match ? [Number(match[1]), Number(match[2])] : null;
}

export function checkServerVersion(serverVersion: string | undefined | null, clientVersion: string = CLIENT_VERSION): VersionVerdict {
  const client = majorMinor(clientVersion);
  if (!client) return { ok: true };
  const server = majorMinor(serverVersion);
  const shown = server ? (serverVersion as string).trim() : null;
  if (!server) return { ok: false, reason: "older", clientVersion, serverVersion: null };
  if (server[0] === client[0] && server[1] === client[1]) return { ok: true };
  const serverIsOlder = server[0] < client[0] || (server[0] === client[0] && server[1] < client[1]);
  return { ok: false, reason: serverIsOlder ? "older" : "newer", clientVersion, serverVersion: shown };
}

/** The sentence that says what is wrong and what to do about it. */
export function versionAdvice(verdict: Extract<VersionVerdict, { ok: false }>, desktop: boolean): { title: string; body: string } {
  const server = verdict.serverVersion ? `version ${verdict.serverVersion}` : "a version that does not report itself (earlier than 0.4.3)";
  if (verdict.reason === "older") {
    return {
      title: "This server is too old for this app",
      body: `This app is version ${verdict.clientVersion} and the server is ${server}. Ask the person who runs the server to update it to ${verdict.clientVersion.replace(/\.\d+$/, ".x")}${desktop ? ", or install the app that matches the server" : ""}.`,
    };
  }
  return {
    title: "This app is too old for this server",
    body: `This app is version ${verdict.clientVersion} and the server is ${server}. ${desktop ? "Install the desktop app that matches the server." : "Reload the page; if it stays, clear the browser's cache or ask the person who runs the server to update the web app."}`,
  };
}
