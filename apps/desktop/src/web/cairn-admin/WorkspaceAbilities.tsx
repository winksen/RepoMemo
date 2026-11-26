import { useState } from "react";
import { Sparkle } from "@phosphor-icons/react";
import { listOrganizationAbilitiesForWorkspace, setWorkspaceAbility } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnAbility } from "../../types";
import { Checkbox } from "../../components/ui/checkbox";
import { PanelHeading } from "../../components/ui/panel-heading";
import { showToast } from "../../components/ui/toast";
import { apiMessage } from "../format";
import { useSystemData } from "../system/shared";
import { AbilitiesPanel } from "./AbilitiesPanel";

/**
 * A workspace's abilities (V0.9.106): the ones its organization teaches, each turned on or off for this workspace, and the
 * workspace's own. What is on shows in the chat's + menu and runs with its /command.
 */
export function WorkspaceAbilities({ accessToken, workspaceId, workspaceName }: { accessToken: string; workspaceId: string; workspaceName: string }) {
  const [shared, , reload] = useSystemData<{ ability: CairnAbility; on: boolean }[]>(cairnAdminKeys.abilities("workspace-organization", workspaceId), () => listOrganizationAbilitiesForWorkspace(accessToken, workspaceId), { failure: "The organization's abilities could not be loaded." });
  const [isBusy, setIsBusy] = useState(false);
  async function change(ability: CairnAbility, on: boolean) {
    setIsBusy(true);
    try {
      await setWorkspaceAbility(accessToken, workspaceId, ability.id, on);
      showToast("success", on ? `${ability.name} is on in this workspace.` : `${ability.name} is off in this workspace.`);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The ability could not be changed.")); }
    finally { setIsBusy(false); }
  }
  return <>
    {shared && shared.length ? <section className="shared-settings-group">
      <PanelHeading description="Taught by your organization; turn off the ones this workspace does not want" icon={Sparkle} title="Organization abilities" />
      {shared.map(({ ability, on }) => <Checkbox checked={on && ability.enabled} description={ability.enabled ? `${ability.command} · ${ability.description || `version ${ability.version}`}` : `${ability.command} · turned off by the organization`} disabled={isBusy || !ability.enabled} key={ability.id} label={ability.name} onCheckedChange={(next) => void change(ability, next)} />)}
    </section> : null}
    <AbilitiesPanel accessToken={accessToken} canChange scope="workspace" scopeId={workspaceId} scopeName={workspaceName} />
  </>;
}
