import { useEffect, useState } from "react";
import { createSharedWorkspace, saveSharedWorkspaceColor, saveSharedWorkspaceIcon } from "../lib/sharedApi";
import type { Organization, SharedWorkspace } from "../types";
import { apiMessage } from "../web/format";
import { useTemplateCatalogue } from "../web/workspace/data";
import { TemplatePicker } from "./TemplatePicker";
import { WorkspaceColorPicker, WorkspaceIconPicker } from "./WorkspaceLookPickers";
import { DEFAULT_WORKSPACE_ICON, WorkspaceIcon } from "../web/workspaceIcons";
import { workspaceColorProps } from "../web/workspaceColors";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Dropdown } from "./ui/dropdown";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { Switch } from "./ui/switch";
import { showToast } from "./ui/toast";

/** Organizations where this person may create workspaces. */
export const creatableOrganizations = (organizations: Organization[]) => organizations.filter((organization) => organization.role === "owner" || organization.role === "admin");

/**
 * *New workspace*: the organization (already chosen when the dialog is opened from one, a list otherwise), the name,
 * its icon and main color, the template and whether access is inherited or restricted to the people added one by one. Opened from the **+** menu and from an organization's page.
 */
export function NewWorkspaceDialog({ accessToken, defaultOrganizationId, onClose, onCreated, open, organizations }: {
  accessToken: string;
  defaultOrganizationId?: string;
  onClose: () => void;
  onCreated: (workspace: SharedWorkspace) => void;
  open: boolean;
  organizations: Organization[];
}) {
  const choices = creatableOrganizations(organizations);
  const [organizationId, setOrganizationId] = useState("");
  const [name, setName] = useState("");
  const [templateId, setTemplateId] = useState("general");
  const [restricted, setRestricted] = useState(false);
  const [icon, setIcon] = useState(DEFAULT_WORKSPACE_ICON);
  const [color, setColor] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [templates] = useTemplateCatalogue(accessToken, open);

  // Each time the dialog opens: the organization it was opened from, else the first one that allows it; nothing typed yet.
  useEffect(() => {
    if (!open) return;
    setOrganizationId(choices.some((organization) => organization.id === defaultOrganizationId) ? defaultOrganizationId! : choices[0]?.id ?? "");
    setName(""); setTemplateId("general"); setRestricted(false); setIcon(DEFAULT_WORKSPACE_ICON); setColor(null);
  }, [open]);

  async function create() {
    if (!organizationId || !name.trim()) return;
    setBusy(true);
    try {
      const workspace = await createSharedWorkspace(accessToken, organizationId, name.trim(), templates.length > 1 ? templateId : undefined, restricted);
      // The icon and the color are settings of the new workspace, saved once it exists; a failure leaves it created with the defaults.
      const settings: Record<string, unknown> = { ...workspace.workspace.settings };
      const failed: string[] = [];
      if (icon !== DEFAULT_WORKSPACE_ICON) {
        try { settings.icon = (await saveSharedWorkspaceIcon(accessToken, workspace.workspace.id, icon)).icon; } catch { failed.push("icon"); }
      }
      if (color) {
        try { settings.color = (await saveSharedWorkspaceColor(accessToken, workspace.workspace.id, color)).color; } catch { failed.push("color"); }
      }
      workspace.workspace = { ...workspace.workspace, settings };
      showToast("success", `${workspace.workspace.name} was created.`);
      if (failed.length) showToast("warning", `The ${failed.join(" and ")} could not be saved. Choose ${failed.length > 1 ? "them" : "it"} again in the workspace's Settings.`);
      onCreated(workspace);
      onClose();
    } catch (requestError) { showToast("error", apiMessage(requestError)); } finally { setBusy(false); }
  }

  if (!choices.length) {
    return <Dialog description="Only an organization's owners and administrators can create its workspaces. Ask one of them, or create an organization of your own." footer={<DialogCancel label="Close" onClick={onClose} />} onClose={onClose} open={open} title="New workspace" />;
  }
  const organization = choices.find((entry) => entry.id === organizationId);
  const asName = `${organization?.name ?? "Organization"} / ${name.trim() || "Workspace name"}`;
  return <Dialog
    className="shared-new-workspace-dialog"
    description="A bounded memory space for a repository, a system or an initiative."
    footer={<><DialogCancel onClick={onClose} /><Button busy={busy} disabled={!organizationId || !name.trim()} type="submit" variant="main">Create workspace</Button></>}
    onClose={onClose}
    onSubmit={() => void create()}
    open={open}
    size="lg"
    title="New workspace"
  >
    <div className="shared-new-workspace-row">
      <Field label="Organization">{(control) => <Dropdown disabled={busy} id={control.id} onValueChange={setOrganizationId} options={choices.map((entry) => ({ label: entry.name, value: entry.id }))} value={organizationId} />}</Field>
      <Field label="Workspace name" required><Input autoFocus disabled={busy} onChange={(event) => setName(event.target.value)} placeholder="Payments platform" required value={name} /></Field>
    </div>
    <p className="shared-new-workspace-as" aria-live="polite">
      <span className="shared-rail-chip" {...workspaceColorProps(color)}><WorkspaceIcon size={20} weight="fill" workspace={{ settings: { icon } }} /></span>
      <span>Workspace will be created as <strong>{asName}</strong></span>
    </p>
    <div className="shared-new-workspace-look">
      <div><strong>Icon</strong><WorkspaceIconPicker disabled={busy} onChange={setIcon} value={icon} /></div>
      <div><strong>Main color</strong><WorkspaceColorPicker disabled={busy} onChange={setColor} value={color} /></div>
    </div>
    <TemplatePicker disabled={busy} name="new-workspace-template" onChange={setTemplateId} templates={templates} value={templateId} />
    <div className="shared-access-choice">
      <Switch aria-label="Restrict access to the people you add" checked={restricted} disabled={busy} onCheckedChange={setRestricted} />
      <div>
        <strong>{restricted ? "Restricted access" : "Access inherited from the organization"}</strong>
        <span>{restricted ? "Only the people you add one by one can open this workspace." : `Everyone in ${organization?.name ?? "the organization"} can open it, with the role they have there.`}</span>
      </div>
    </div>
  </Dialog>;
}
