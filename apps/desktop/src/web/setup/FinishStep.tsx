import { useState } from "react";
import { IconArrowLeft as ArrowLeft, IconFlag as Flag } from "@tabler/icons-react";
import { completeServerSetup } from "../../lib/sharedApi";
import type { SharedSession, SystemSetting } from "../../types";
import { Button } from "../../components/ui/button";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";
import { StepHeading } from "./StepFrame";
import { settingValueLabel } from "./SettingsStep";

export function FinishStep({ accessToken, changedSettings, created, onBack, onFinished, session, warnings }: {
  accessToken: string;
  changedSettings: SystemSetting[];
  created: { organization: string; workspace: string | null } | null;
  onBack: () => void;
  onFinished: () => void;
  session?: SharedSession;
  warnings: number;
}) {
  const [isBusy, setIsBusy] = useState(false);
  async function finish() {
    setIsBusy(true);
    try {
      await completeServerSetup(accessToken);
      showToast("success", "The server is set up. Welcome to RepoMemo.");
      onFinished();
    } catch (error) { showToast("error", apiMessage(error, "The setup could not be finished.")); setIsBusy(false); }
  }
  return <>
    <StepHeading eyebrow="Step 7 of 7" title="Review and finish">
      Finishing opens the server to the people you invite, under the settings above. This setup page then disappears for good; everything can still be changed in System.
    </StepHeading>
    <dl className="rm-system-facts">
      <div><dt>System administrator</dt><dd>{session?.user.email ?? session?.user.display_name ?? "You"}</dd></div>
      <div><dt>Server check</dt><dd>{warnings ? `${warnings} point${warnings === 1 ? "" : "s"} to look at` : "No warnings"}</dd></div>
      <div><dt>Settings changed</dt><dd>{changedSettings.length ? changedSettings.map((setting) => `${setting.label}: ${settingValueLabel(setting, setting.value)}`).join(" · ") : "Server defaults kept"}</dd></div>
      <div><dt>First workspace</dt><dd>{created ? `${created.organization}${created.workspace ? ` / ${created.workspace}` : ""}` : "Skipped"}</dd></div>
    </dl>
    <div className="rm-setup-actions">
      <Button disabled={isBusy} onClick={onBack} variant="secondary"><ArrowLeft size={ICON_SIZE.md} /> Back</Button>
      <Button busy={isBusy} onClick={() => void finish()} variant="main"><Flag size={ICON_SIZE.md} /> Finish setup</Button>
    </div>
  </>;
}
