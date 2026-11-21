import { useEffect, useState } from "react";
import { createSharedWorkspace } from "../lib/sharedApi";
import type { Organization, SharedWorkspace } from "../types";
import { apiMessage } from "../web/format";
import { useTemplateCatalogue } from "../web/workspace/data";
import { WorkspaceIcon } from "../web/workspaceIcons";
import { DEFAULT_WORKSPACE_SETUP, type WorkspaceSetup, WorkspaceSetupFields, applyWorkspaceLook, lookWarning } from "./workspaceSetup";
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
  const [setup, setSetup] = useState<WorkspaceSetup>(DEFAULT_WORKSPACE_SETUP);
  const [busy, setBusy] = useState(false);
  const [templates] = useTemplateCatalogue(accessToken, open);

  // Each time the dialog opens: the organization it was opened from, else the first one that allows it; nothing typed yet.
  useEffect(() => {
    if (!open) return;
    setOrganizationId(choices.some((organization) => organization.id === defaultOrganizationId) ? defaultOrganizationId! : choices[0]?.id ?? "");
    setName(""); setSetup(DEFAULT_WORKSPACE_SETUP);
  }, [open]);

  async function create() {
    if (!organizationId || !name.trim()) return;
    setBusy(true);
    try {
      const created = await createSharedWorkspace(accessToken, organizationId, name.trim(), templates.length > 1 ? setup.templateId : undefined, setup.restricted);
      // The icon and the color are settings of the new workspace, saved once it exists.
      const { workspace, failed } = await applyWorkspaceLook(accessToken, created, setup);
      showToast("success", `${workspace.workspace.name} was created.`);
      if (failed.length) showToast("warning", lookWarning(failed));
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
      <span className="shared-rail-chip" {...workspaceColorProps(setup.color)}><WorkspaceIcon size={20} weight="fill" workspace={{ settings: { icon: setup.icon } }} /></span>
      <span>Workspace will be created as <strong>{asName}</strong></span>
    </p>
    <WorkspaceSetupFields busy={busy} onChange={setSetup} organizationName={organization?.name ?? ""} setup={setup} templates={templates} />
  </Dialog>;
}
