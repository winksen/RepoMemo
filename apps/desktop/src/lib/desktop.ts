import { invoke } from "@tauri-apps/api/core";

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
