/**
 * What a person typed as a server address, checked the way the desktop shell checks it
 * (`normalize_server_url` in src-tauri/src/connection.rs, which is the one that decides): the
 * answer here only saves a round trip and words the problem. Keep the two in step.
 */
export type ServerAddress = { ok: true; origin: string } | { ok: false; message: string };

const PRIVATE_V4 = [/^127\./, /^10\./, /^192\.168\./, /^172\.(1[6-9]|2\d|3[01])\./, /^169\.254\./];

function isLocalHost(hostname: string): boolean {
  const host = hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (host.includes(":")) return host === "::1" || /^f[cd][0-9a-f]{2}:/.test(host) || /^fe[89ab][0-9a-f]:/.test(host);
  if (/^\d+\.\d+\.\d+\.\d+$/.test(host)) return PRIVATE_V4.some((pattern) => pattern.test(host));
  return host === "localhost" || !host.includes(".") || [".localhost", ".local", ".lan", ".internal", ".home.arpa"].some((suffix) => host.endsWith(suffix));
}

export function normalizeServerAddress(input: string): ServerAddress {
  const typed = input.trim();
  if (!typed) return { ok: false, message: "Enter the address of your RepoMemo server." };
  let url: URL;
  try {
    url = new URL(typed.includes("://") ? typed : `https://${typed}`);
  } catch {
    return { ok: false, message: "That is not a web address." };
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return { ok: false, message: "The address must start with https:// (or http:// on your own network)." };
  if (!url.hostname) return { ok: false, message: "The address has no host name." };
  if (url.username || url.password) return { ok: false, message: "Leave the user name and password out of the address; you sign in on the next screen." };
  if (url.search || url.hash || (url.pathname !== "/" && url.pathname !== "")) return { ok: false, message: "Enter only the server's address, such as https://repomemo.example.com." };
  if (url.protocol === "http:" && !isLocalHost(url.hostname)) return { ok: false, message: "A server on the internet must use https://. Plain http:// is only for a server on your own network." };
  return { ok: true, origin: url.origin };
}
