import { useEffect, useState } from "react";
import { appUpdates, type UpdateInfo } from "../lib/desktop";
import { readUpdateChoice } from "../lib/updateChoice";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { showToast } from "./ui/toast";
import { apiMessage } from "../web/format";

/**
 * Looks for a newer version of the app once, when the client is up (V0.4.56), and offers it. The shell looks only where the
 * administrator's file says, or at the project's releases when the person turned that on; an update whose signature does not check is
 * refused by the shell and shown as nothing. Nothing is installed without the person's answer.
 */
export function UpdateOffer() {
  const [offer, setOffer] = useState<UpdateInfo | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);

  useEffect(() => {
    let cancelled = false;
    Promise.resolve().then(() => appUpdates.check(readUpdateChoice())).then((found) => { if (!cancelled) setOffer(found); }, () => undefined);
    return () => { cancelled = true; };
  }, []);

  async function install() {
    setIsInstalling(true);
    try {
      // The app restarts itself on the new version; nothing after this runs if it works.
      await appUpdates.install();
    } catch (error) {
      // The shell answers with a sentence of its own (a signature that does not match, a download that failed).
      showToast("error", typeof error === "string" && error.trim() ? error : apiMessage(error, "The update could not be installed."));
      setIsInstalling(false);
    }
  }

  return (
    <Dialog
      description={offer?.notes ? `${offer.notes.split("\n")[0]}` : "A newer version of the RepoMemo app is ready. It is checked against its signature before it is installed, and the app restarts."}
      footer={<>
        <DialogCancel onClick={() => setOffer(null)} />
        <Button busy={isInstalling} onClick={() => void install()} type="button" variant="main">Install and restart</Button>
      </>}
      onClose={() => setOffer(null)}
      open={offer !== null}
      title={offer ? `Version ${offer.version} is available` : "Update"}
    />
  );
}
