import { useState } from "react";
import { Sparkle } from "@phosphor-icons/react";
import { listCairnPlaybooks, listOrganizationAbilitiesForWorkspace, makeAbilityFromPlaybook, setWorkspaceAbility } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnAbility, CairnPlaybook } from "../../types";
import { Button } from "../../components/ui/button";
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
  const [playbooks, , reloadPlaybooks] = useSystemData<CairnPlaybook[]>(cairnAdminKeys.playbooks(workspaceId), () => listCairnPlaybooks(accessToken, workspaceId), { failure: "The playbooks could not be loaded." });
  const [isBusy, setIsBusy] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  async function makeAbility(playbook: CairnPlaybook) {
    setIsBusy(true);
    try {
      const ability = await makeAbilityFromPlaybook(accessToken, workspaceId, playbook.artifact_id);
      showToast("success", `${playbook.name} is now the ability ${ability.command}.`);
      await reloadPlaybooks();
      setRefreshKey((key) => key + 1);
    } catch (error) { showToast("error", apiMessage(error, "The playbook could not be made an ability.")); }
    finally { setIsBusy(false); }
  }
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
    {playbooks && playbooks.length ? <section className="shared-settings-group">
      <PanelHeading description="Verified playbooks of this workspace. An ability made from one keeps the link, so each shows the other" icon={Sparkle} title="Playbooks" />
      {playbooks.map((playbook) => <div className="rm-system-budget-row" key={playbook.artifact_id}>
        <span><strong>{playbook.name}</strong><small className="shared-muted-copy"> · {playbook.description}</small></span>
        {playbook.ability_command ? <span className="shared-muted-copy">Ability {playbook.ability_command}</span> : <Button busy={isBusy} onClick={() => void makeAbility(playbook)} type="button" variant="secondary">Make an ability</Button>}
      </div>)}
    </section> : null}
    <AbilitiesPanel accessToken={accessToken} canChange playbookNames={Object.fromEntries((playbooks ?? []).map((playbook) => [playbook.artifact_id, playbook.name]))} refreshKey={refreshKey} scope="workspace" scopeId={workspaceId} scopeName={workspaceName} />
  </>;
}
