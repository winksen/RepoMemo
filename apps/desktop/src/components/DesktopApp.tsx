import { useEffect, useState } from "react";
import { SharedWebApp } from "../SharedWebApp";
import { LoadingCanvas } from "../web/common";
import { desktopServer, installExternalLinkHandler } from "../lib/desktop";
import { DesktopConnect, pointAtServer } from "./DesktopConnect";

type Phase = "starting" | "connect" | "server";

/** Keys the earlier desktop app kept in this window's storage for the local workspace and its move (removed in V0.3.5). */
const RETIRED_KEYS = ["repomemo.desktop.mode", "repomemo.desktop.handoff"];

function forgetRetiredChoices() {
  try {
    for (const key of RETIRED_KEYS) window.localStorage.removeItem(key);
  } catch { /* only a convenience */ }
}

/**
 * What the desktop window shows (V0.2.49). With a server chosen it shows the web client against that
 * server, the same pages the browser shows; without one it asks for an address. The local workspace of
 * the earlier desktop app is gone (V0.3.5).
 */
export function DesktopApp() {
  const [phase, setPhase] = useState<Phase>("starting");

  useEffect(() => installExternalLinkHandler(), []);

  useEffect(() => {
    forgetRetiredChoices();
    let cancelled = false;
    (async () => {
      try {
        const origin = await desktopServer.current();
        if (cancelled) return;
        if (origin) {
          await pointAtServer(origin);
          if (!cancelled) setPhase("server");
        } else setPhase("connect");
      } catch {
        if (!cancelled) setPhase("connect");
      }
    })();
    return () => { cancelled = true; };
  }, []);

  if (phase === "starting") return <LoadingCanvas label="Starting RepoMemo" />;
  if (phase === "server") return <SharedWebApp />;
  return <DesktopConnect onConnected={() => setPhase("server")} />;
}
