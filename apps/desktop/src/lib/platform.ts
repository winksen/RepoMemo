import { isDesktopRuntime } from "./desktop";

/**
 * What the window the client runs in can do (V0.4.44). A component asks for a capability here and never asks which runtime it is in,
 * so the desktop app needs no branch inside a component: the difference is made once, below.
 */
export const platform = {
  /** The page can open a new tab and hand a link to another program (a browser can; the app's window cannot). */
  opensTabs: !isDesktopRuntime,
  /** The person chose a server in this window and can choose another (only the app has one to forget). */
  choosesServer: isDesktopRuntime,
  /** The client was installed on this computer, so a version that does not fit the server's means updating it, not reloading the page. */
  installedApp: isDesktopRuntime,
};
