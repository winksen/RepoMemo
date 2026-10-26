import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Whether this page runs inside the desktop app's window (it then talks to a chosen server through the shell). */
export const isDesktopRuntime = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** The server a desktop run talks to; the shell checks every address and lets the window call only that one. */
export const desktopServer = {
  /** The address this run talks to: the one the app was packaged for, else the one saved by an earlier run, or null. */
  current: () => invoke<string | null>("current_server"),
  /** Whether the app was packaged for one server (V0.4.46), which then cannot be changed. */
  locked: () => invoke<boolean>("server_locked"),
  /** Checks an address and allows the window to call it. Nothing is kept until `remember`. */
  prepare: (address: string) => invoke<string>("prepare_server", { address }),
  /** Keeps the prepared server for the next start. */
  remember: () => invoke<void>("remember_server"),
  /** Forgets the server and its session, and restarts the app so it asks again. */
  forget: () => invoke<void>("forget_server"),
};

/** The session's refresh token, kept in the operating system's credential store and nowhere else. */
export const sessionStore = {
  save: (refreshToken: string) => invoke<void>("save_session", { refreshToken }),
  load: () => invoke<string | null>("load_session"),
  clear: () => invoke<void>("clear_session"),
};

/** Opens a web page in the system browser (the window itself never leaves the app). */
export function openExternal(url: string): Promise<void> {
  return invoke<void>("open_external", { url });
}

/** Links that open a new tab in a browser go to the system browser here. */
export function installExternalLinkHandler(): () => void {
  const onClick = (event: MouseEvent) => {
    if (event.defaultPrevented || event.button !== 0) return;
    const anchor = (event.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
    if (!anchor || anchor.target !== "_blank") return;
    const href = anchor.href;
    if (!/^https?:/i.test(href)) return;
    event.preventDefault();
    void openExternal(href).catch(() => undefined);
  };
  document.addEventListener("click", onClick);
  return () => document.removeEventListener("click", onClick);
}

/** What the administrator's configuration file (V0.4.47) says, as far as the page needs it. */
export type DesktopSettings = {
  /** What the app is called (the window's title and the page's). */
  name: string | null;
  /** Where a person asks for help. */
  supportUrl: string | null;
  /** A server to offer on the connect screen, when the file names one without locking it. */
  suggestedServer: string | null;
  /** The file that was found. */
  source: string | null;
  /** Why that file cannot be used; when set, nothing in it is obeyed and the app does not connect. */
  problem: string | null;
};

export const desktopSettings = {
  get: () => invoke<DesktopSettings>("desktop_settings"),
};

/**
 * Makes the window's title the page's (V0.4.51): the client names the page it shows in `document.title` ("Platform · Evidence",
 * with the workspace's name), and a window of the app has no tab to show it in, so the title bar does. Returns the way to stop.
 */
export function followPageTitle(): () => void {
  const window_ = getCurrentWindow();
  const apply = () => void window_.setTitle(document.title).catch(() => undefined);
  const observer = new MutationObserver(apply);
  observer.observe(document.head, { childList: true, subtree: true, characterData: true });
  apply();
  return () => observer.disconnect();
}

/** The bytes of a blob (a webview has `arrayBuffer`; the older readers in a test page do not). */
function readBytes(blob: Blob): Promise<Uint8Array> {
  if (typeof blob.arrayBuffer === "function") return blob.arrayBuffer().then((buffer) => new Uint8Array(buffer));
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(blob);
  });
}

/**
 * Saves a file through the shell's native save dialog (V0.4.52). The bytes go to the shell as the body of the call, not as JSON, and the
 * name goes in a header as a suggestion the person can change. Answers whether the file was saved (`false`: the person cancelled).
 */
export async function saveThroughShell(blob: Blob, filename: string): Promise<boolean> {
  const bytes = await readBytes(blob);
  const saved = await invoke<string | null>("save_file", bytes, { headers: { "x-file-name": encodeURIComponent(filename) } });
  return saved !== null;
}
