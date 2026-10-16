import { type ReactNode, useEffect, useState } from "react";
import { IconServer as Server } from "@tabler/icons-react";
import { SharedWebApp } from "../SharedWebApp";
import { LoadingCanvas } from "../web/common";
import { desktopServer, installExternalLinkHandler } from "../lib/desktop";
import { DesktopConnect, pointAtServer } from "./DesktopConnect";
import { Button } from "./ui/button";

const MODE_KEY = "repomemo.desktop.mode";

function chosenLocal(): boolean {
  try { return window.localStorage.getItem(MODE_KEY) === "local"; } catch { return false; }
}

function rememberMode(local: boolean) {
  try {
    if (local) window.localStorage.setItem(MODE_KEY, "local");
    else window.localStorage.removeItem(MODE_KEY);
  } catch { /* only a convenience */ }
}

type Phase = "starting" | "connect" | "server" | "local";

/**
 * What the desktop window shows (V0.2.49). With a server chosen it shows the web client against that
 * server, the same pages the browser shows; without one it asks for an address. The local workspace of
 * the earlier desktop app stays reachable until it is retired, and is only chosen on purpose.
 */
export function DesktopApp({ local }: { local: ReactNode }) {
  const [phase, setPhase] = useState<Phase>("starting");

  useEffect(() => installExternalLinkHandler(), []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const origin = await desktopServer.current();
        if (cancelled) return;
        if (origin) {
          await pointAtServer(origin);
          if (!cancelled) setPhase("server");
        } else setPhase(chosenLocal() ? "local" : "connect");
      } catch {
        if (!cancelled) setPhase("connect");
      }
    })();
    return () => { cancelled = true; };
  }, []);

  if (phase === "starting") return <LoadingCanvas label="Starting RepoMemo" />;
  if (phase === "server") return <SharedWebApp />;
  if (phase === "local") {
    return <>
      {local}
      <Button className="rm-desktop-switch" onClick={() => { rememberMode(false); setPhase("connect"); }} type="button" variant="secondary"><Server size={15} /> Connect to a server</Button>
    </>;
  }
  return <DesktopConnect onConnected={() => setPhase("server")} onUseLocal={() => { rememberMode(true); setPhase("local"); }} />;
}
