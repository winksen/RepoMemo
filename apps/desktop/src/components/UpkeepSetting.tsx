import { useEffect, useState } from "react";
import type { UpkeepSettings } from "../generated/domain";
import { getSharedUpkeep, setSharedUpkeep } from "../lib/sharedApi";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { apiMessage } from "../web/format";
import { formatRecentTime } from "../web/format";
import { Checkbox } from "./ui/checkbox";
import { showToast } from "./ui/toast";

/** The switch for the weekly memory upkeep (V0.9.56): off until an owner or administrator turns it on. It finds hints and edits nothing. */
export function UpkeepSetting({ accessToken, workspaceId }: { accessToken: string; workspaceId: string }) {
  const [upkeep, setUpkeep] = useState<UpkeepSettings | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let active = true;
    setUpkeep(null);
    getSharedUpkeep(accessToken, workspaceId)
      .then((next) => { if (active) setUpkeep(next); })
      .catch((error) => { if (active) showToast("error", apiMessage(error, "The memory upkeep setting could not be loaded.")); });
    return () => { active = false; };
  }, [accessToken, workspaceId]);

  async function change(enabled: boolean) {
    setBusy(true);
    try {
      setUpkeep(await setSharedUpkeep(accessToken, workspaceId, enabled));
      showToast("success", enabled ? "Memory upkeep is on." : "Memory upkeep is off.");
    } catch (error) {
      showToast("error", apiMessage(error, "The memory upkeep setting could not be saved."));
    } finally {
      setBusy(false);
    }
  }

  return <div className="shared-ai-provider-form">
    <Checkbox
      checked={upkeep?.enabled ?? false}
      description={`Once a week ${ASSISTANT_NAME} looks for cards that may disagree, questions people asked and never got an answer to, and playbooks nobody used for 90 days, and puts what it finds in the review queue. It changes nothing and asks no AI model.${upkeep?.last_run_at ? ` Last run ${formatRecentTime(upkeep.last_run_at)}.` : ""}`}
      disabled={!upkeep || busy}
      label="Weekly memory upkeep"
      onCheckedChange={(enabled) => void change(enabled)}
    />
  </div>;
}
