/**
 * Where the client's requests go out (V0.4.44), the one seam between the web client and the network. `sharedApi` sends every
 * request through the transport it was given and uses no other `fetch`; the browser gives it its own, the desktop app gives it
 * the shell's. Nothing else decides how a request is sent.
 */
export type Transport = typeof fetch;

/** The browser's own `fetch`, with the page's cookies and its content security policy. Looked up when called, so a test can replace it. */
export const browserTransport: Transport = (input, init) => fetch(input, init);

/**
 * The shell's HTTP client (`tauri-plugin-http`), which the window may use only for the server the person chose and which keeps no
 * cookies. A redirect is never followed, so a server cannot send the app to an address the shell did not allow.
 */
export async function shellTransport(): Promise<Transport> {
  const { fetch: pluginFetch } = await import("@tauri-apps/plugin-http");
  return ((input: RequestInfo | URL, init?: RequestInit) => pluginFetch(input, { ...init, maxRedirections: 0 })) as Transport;
}
