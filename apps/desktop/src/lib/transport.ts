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
export async function shellTransport(relay?: Relay): Promise<Transport> {
  const { fetch: pluginFetch } = await import("@tauri-apps/plugin-http");
  return ((input: RequestInfo | URL, init?: RequestInit) => {
    if (!relay) return pluginFetch(input, { ...init, maxRedirections: 0 });
    const { url, headers } = throughRelay(relay, input, init);
    return pluginFetch(url, { ...init, headers, maxRedirections: 0 });
  }) as Transport;
}

/**
 * Where a server with its own certificate authority is reached (V0.4.55): the shell's relay on the loopback, which checks the server's
 * certificate by the administrator's bundle. The page keeps using the server's own address; the transport sends the request to the
 * relay with the run's secret, and sends a request for any other address as it is, where the shell's scope refuses it.
 */
export type Relay = { origin: string; proxy: string; token: string };

const RELAY_HEADER = "x-repomemo-route";

/** The address and headers of a request that goes through the relay (or unchanged, when it is not for the relay's server). */
export function throughRelay(relay: Relay, input: RequestInfo | URL, init?: RequestInit): { url: string; headers: Headers } {
  const original = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
  const headers = new Headers(init?.headers ?? (typeof input === "object" && "headers" in input ? input.headers : undefined));
  if (original !== relay.origin && !original.startsWith(`${relay.origin}/`) && !original.startsWith(`${relay.origin}?`)) return { url: original, headers };
  headers.set(RELAY_HEADER, relay.token);
  return { url: `${relay.proxy}${original.slice(relay.origin.length)}`, headers };
}
