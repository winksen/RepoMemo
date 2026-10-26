import { useEffect, useState } from "react";
import { SharedWebApp } from "../SharedWebApp";
import { setNotifier } from "../lib/notifier";
import { setFileSaver } from "../lib/saveBlob";
import { LoadingCanvas } from "../web/common";
import { desktopServer, desktopSettings, followPageTitle, installExternalLinkHandler, saveThroughShell, shellNotifier, type DesktopSettings } from "../lib/desktop";
import { pointAtServer } from "../lib/desktopConnection";
import { DesktopConfigProblem } from "./DesktopConfigProblem";
import { DesktopConnect } from "./DesktopConnect";

type Phase = "starting" | "connect" | "server" | "problem";

/** What the app is told when it has no configuration file, or the shell cannot say. */
const NO_SETTINGS: DesktopSettings = { name: null, supportUrl: null, suggestedServer: null, source: null, problem: null };

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
 * the earlier desktop app is gone (V0.3.5). An administrator's configuration file (V0.4.47) can name the
 * server and the app; one that cannot be used is shown instead of everything else.
 */
export function DesktopApp() {
  const [phase, setPhase] = useState<Phase>("starting");
  const [settings, setSettings] = useState<DesktopSettings>(NO_SETTINGS);

  useEffect(() => installExternalLinkHandler(), []);
  useEffect(() => followPageTitle(), []);
  // Notices and the badge go through the shell (V0.4.53).
  useEffect(() => {
    setNotifier(shellNotifier);
    return () => setNotifier(null);
  }, []);
  // Files are saved through the shell's dialog here (V0.4.52).
  useEffect(() => {
    setFileSaver(saveThroughShell);
    return () => setFileSaver(null);
  }, []);

  useEffect(() => {
    forgetRetiredChoices();
    let cancelled = false;
    (async () => {
      const found = await desktopSettings.get().catch(() => NO_SETTINGS);
      if (cancelled) return;
      setSettings(found);
      if (found.name) document.title = found.name;
      if (found.problem) {
        setPhase("problem");
        return;
      }
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
  if (phase === "problem") return <DesktopConfigProblem problem={settings.problem ?? ""} />;
  return <DesktopConnect onConnected={() => setPhase("server")} suggestedServer={settings.suggestedServer} supportUrl={settings.supportUrl} />;
}
