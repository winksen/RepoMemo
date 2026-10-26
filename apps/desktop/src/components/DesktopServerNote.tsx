import { useEffect, useState } from "react";
import { IconServer as Server } from "@tabler/icons-react";
import { sharedApiUrl } from "../lib/sharedApi";
import { desktopServer } from "../lib/desktop";
import { platform } from "../lib/platform";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

/** In the desktop app: which server this window is connected to, and the way to choose another. Nothing in a browser. */
export function DesktopServerNote({ signedIn }: { signedIn: boolean }) {
  const [asking, setAsking] = useState(false);
  const [isChanging, setIsChanging] = useState(false);
  // An app packaged for one server (V0.4.46) shows it and offers no way to change it; until the shell says, it offers none.
  const [locked, setLocked] = useState(true);
  useEffect(() => {
    if (!platform.choosesServer) return;
    let cancelled = false;
    desktopServer.locked().then((value) => { if (!cancelled) setLocked(value); }, () => { if (!cancelled) setLocked(false); });
    return () => { cancelled = true; };
  }, []);
  if (!platform.choosesServer) return null;
  const host = sharedApiUrl.replace(/^https?:\/\//, "");

  async function change() {
    setIsChanging(true);
    try {
      // Forgets the server and its stored session, then the app starts again and asks for an address.
      await desktopServer.forget();
    } catch (error) {
      showToast("error", apiMessage(error, "The server could not be changed."));
      setIsChanging(false);
    }
  }

  return <>
    <div className="rm-desktop-server">
      <span><Server size={ICON_SIZE.sm} /> <span className="rm-desktop-server-host" title={sharedApiUrl}>{host}</span></span>
      {locked ? null : <Button onClick={() => setAsking(true)} type="button" variant="secondary">Change server</Button>}
    </div>
    <Dialog
      description={signedIn
        ? "This signs you out on this computer and forgets the server. Your account and everything in it stay on the server."
        : "The app forgets this server and asks for another address."}
      footer={<>
        <DialogCancel onClick={() => setAsking(false)} />
        <Button disabled={isChanging} onClick={() => void change()} type="button" variant="main">Change server</Button>
      </>}
      onClose={() => setAsking(false)}
      open={asking}
      title="Change server?"
    />
  </>;
}
