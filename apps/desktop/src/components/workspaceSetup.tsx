import { saveSharedWorkspaceColor, saveSharedWorkspaceIcon } from "../lib/sharedApi";
import type { SharedWorkspace, TemplateSummary } from "../types";
import { DEFAULT_WORKSPACE_ICON } from "../web/workspaceIcons";
import { TemplatePicker } from "./TemplatePicker";
import { WorkspaceColorPicker, WorkspaceIconPicker } from "./WorkspaceLookPickers";
import { Switch } from "./ui/switch";

/** What is chosen for a workspace that is about to be created, beyond its name. */
export interface WorkspaceSetup { icon: string; color: string | null; templateId: string; restricted: boolean }

export const DEFAULT_WORKSPACE_SETUP: WorkspaceSetup = { icon: DEFAULT_WORKSPACE_ICON, color: null, templateId: "general", restricted: false };

/**
 * The icon and the color (side by side, in the same halves as the fields above them), the template cards and the access
 * choice: shared by the New workspace and New organization dialogs.
 */
export function WorkspaceSetupFields({ busy, onChange, organizationName, setup, templates }: {
  busy: boolean;
  onChange: (setup: WorkspaceSetup) => void;
  organizationName: string;
  setup: WorkspaceSetup;
  templates: TemplateSummary[];
}) {
  return <>
    <div className="shared-new-workspace-look">
      <div><strong>Icon</strong><WorkspaceIconPicker disabled={busy} onChange={(icon) => onChange({ ...setup, icon })} value={setup.icon} /></div>
      <div><strong>Main color</strong><WorkspaceColorPicker disabled={busy} onChange={(color) => onChange({ ...setup, color })} value={setup.color} /></div>
    </div>
    <TemplatePicker disabled={busy} name="new-workspace-template" onChange={(templateId) => onChange({ ...setup, templateId })} templates={templates} value={setup.templateId} />
    <div className="shared-access-choice">
      <Switch aria-label="Restrict access to the people you add" checked={setup.restricted} disabled={busy} onCheckedChange={(restricted) => onChange({ ...setup, restricted })} />
      <div>
        <strong>{setup.restricted ? "Restricted access" : "Access inherited from the organization"}</strong>
        <span>{setup.restricted ? "Only the people you add one by one can open this workspace." : `Everyone in ${organizationName || "the organization"} can open it, with the role they have there.`}</span>
      </div>
    </div>
  </>;
}

/**
 * Saves the icon and the color that were chosen, once the workspace exists, and returns it with them in its settings. A
 * failure leaves the workspace as created and says which choice to make again.
 */
export async function applyWorkspaceLook(accessToken: string, workspace: SharedWorkspace, setup: WorkspaceSetup): Promise<{ workspace: SharedWorkspace; failed: string[] }> {
  const settings: Record<string, unknown> = { ...workspace.workspace.settings };
  const failed: string[] = [];
  if (setup.icon !== DEFAULT_WORKSPACE_ICON) {
    try { settings.icon = (await saveSharedWorkspaceIcon(accessToken, workspace.workspace.id, setup.icon)).icon; } catch { failed.push("icon"); }
  }
  if (setup.color) {
    try { settings.color = (await saveSharedWorkspaceColor(accessToken, workspace.workspace.id, setup.color)).color; } catch { failed.push("color"); }
  }
  return { workspace: { ...workspace, workspace: { ...workspace.workspace, settings } }, failed };
}

export const lookWarning = (failed: string[]) => `The ${failed.join(" and ")} could not be saved. Choose ${failed.length > 1 ? "them" : "it"} again in the workspace's Settings.`;
