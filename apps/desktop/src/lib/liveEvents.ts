import { createContext, useContext, useEffect, useMemo, useRef, useState } from "react";
import { openSharedEventStream } from "./sharedApi";
import type { LiveEvent } from "../types";

type Listener = (event: LiveEvent) => void;

/** One open event stream that several components can listen to. */
export interface LiveEventHub {
  /** True while the stream is open. Pages keep a slow poll as a fallback
   *  while it is not. */
  connected: boolean;
  subscribe: (listener: Listener) => () => void;
}

/** Waits before reconnecting, longer after each failed attempt. */
const RECONNECT_DELAYS_MS = [1_000, 2_000, 5_000, 10_000, 30_000];

/** The stream of the page being shown, for panels inside it. */
export const LiveEventsContext = createContext<LiveEventHub | null>(null);

/** Keeps an event stream open for as long as the component is mounted, and
 *  reconnects when it ends (the server closes it when the token expires).
 *  After a reconnect, listeners get a `resync` because events may have been
 *  missed in between. Pass `null` to open nothing. */
export function useLiveEventHub(path: string | null, accessToken: string): LiveEventHub {
  const [connected, setConnected] = useState(false);
  const listeners = useRef(new Set<Listener>());
  // The newest token is used on reconnect without reopening a healthy stream.
  const token = useRef(accessToken);
  token.current = accessToken;

  useEffect(() => {
    if (!path) return;
    const controller = new AbortController();
    const dispatch = (event: LiveEvent) => {
      for (const listener of [...listeners.current]) {
        try { listener(event); } catch { /* one listener must not stop the others */ }
      }
    };

    void (async () => {
      let failures = 0;
      let wasConnected = false;
      while (!controller.signal.aborted) {
        try {
          const response = await openSharedEventStream(path, token.current, controller.signal);
          if (!response.ok || !response.body) throw new Error(`Event stream answered ${response.status}`);
          setConnected(true);
          failures = 0;
          if (wasConnected) dispatch({ type: "resync" });
          wasConnected = true;
          await readEventStream(response.body, dispatch, controller.signal);
        } catch {
          failures += 1;
        }
        setConnected(false);
        if (controller.signal.aborted) return;
        const delay = RECONNECT_DELAYS_MS[Math.min(failures, RECONNECT_DELAYS_MS.length - 1)];
        await new Promise<void>((resolve) => {
          const timer = window.setTimeout(resolve, delay);
          controller.signal.addEventListener("abort", () => { window.clearTimeout(timer); resolve(); }, { once: true });
        });
      }
    })();

    return () => { controller.abort(); setConnected(false); };
  }, [path]);

  return useMemo(() => ({
    connected,
    subscribe: (listener: Listener) => {
      listeners.current.add(listener);
      return () => { listeners.current.delete(listener); };
    },
  }), [connected]);
}

/** Calls `listener` for every event of `hub`, or of the page's stream from
 *  `LiveEventsContext` when no hub is given. Returns whether the stream is
 *  open; false when there is none. */
export function useLiveEvents(listener: Listener, hub?: LiveEventHub | null): boolean {
  const contextHub = useContext(LiveEventsContext);
  const source = hub ?? contextHub;
  const latest = useRef(listener);
  latest.current = listener;
  const subscribe = source?.subscribe;
  useEffect(() => subscribe?.((event) => latest.current(event)), [subscribe]);
  return source?.connected ?? false;
}

/** Reads `event:`/`data:` blocks until the stream ends. Comments (the
 *  server's keep-alives) and blocks without JSON data are ignored. */
async function readEventStream(body: ReadableStream<Uint8Array>, dispatch: Listener, signal: AbortSignal) {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  signal.addEventListener("abort", () => { void reader.cancel().catch(() => undefined); }, { once: true });
  for (;;) {
    const { done, value } = await reader.read();
    if (done) return;
    buffer += decoder.decode(value, { stream: true }).replace(/\r\n?/g, "\n");
    let boundary = buffer.indexOf("\n\n");
    while (boundary >= 0) {
      const block = buffer.slice(0, boundary);
      buffer = buffer.slice(boundary + 2);
      const data = block
        .split("\n")
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.slice(5).replace(/^ /, ""))
        .join("\n");
      if (data) {
        try {
          const event = JSON.parse(data) as LiveEvent;
          if (event && typeof event.type === "string") dispatch(event);
        } catch { /* not JSON; skip it */ }
      }
      boundary = buffer.indexOf("\n\n");
    }
  }
}
