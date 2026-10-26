/**
 * `repomemo://` links (V0.4.54), as a page of the web client makes them. A link is a request to show one of the client's own pages
 * in the desktop app: `repomemo://open?server=<origin>&path=<path>`. The app takes only the client's own paths and only for the
 * server it is connected to; the rules are the app's (`src-tauri/src/deeplink.rs`, whose list of areas a test compares with this one).
 */

/** The first parts of the paths the client has pages under (its router's). */
export const APP_AREAS = ["workspaces", "dashboard", "profile", "notifications", "system"] as const;
const MAX_PATH_CHARS = 2048;

// eslint-disable-next-line no-control-regex
const CONTROL_OR_BACKSLASH = /[\u0000-\u001f\u007f-\u009f\\]/;
// eslint-disable-next-line no-control-regex
const FOLDER_OR_CONTROL = /[/\\\u0000-\u001f\u007f-\u009f]/;

/** Whether this is a path of the client, with an optional query and fragment, and nothing that leaves it. */
export function isAppPath(path: string): boolean {
  if ([...path].length > MAX_PATH_CHARS || !path.startsWith("/") || path.startsWith("//")) return false;
  if (CONTROL_OR_BACKSLASH.test(path)) return false;
  const parts = path.split(/[?#]/)[0].split("/").filter(Boolean);
  if (parts.length === 0 || !(APP_AREAS as readonly string[]).includes(parts[0])) return false;
  return parts.every((part) => {
    let decoded: string;
    try {
      decoded = decodeURIComponent(part);
    } catch {
      return false;
    }
    return decoded !== ".." && decoded !== "." && !FOLDER_OR_CONTROL.test(decoded);
  });
}

/** The link that opens `path` of `server` in the app, or `null` when either is not something a link may carry. */
export function appLinkFor(server: string, path: string): string | null {
  if (!isAppPath(path)) return null;
  let origin: string;
  try {
    const url = new URL(server);
    if (url.protocol !== "https:" && url.protocol !== "http:") return null;
    origin = url.origin;
  } catch {
    return null;
  }
  return `repomemo://open?server=${encodeURIComponent(origin)}&path=${encodeURIComponent(path)}`;
}
