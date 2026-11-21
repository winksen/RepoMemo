import { useEffect, useState } from "react";
import { createSharedWorkspace } from "../lib/sharedApi";
import type { Organization, SharedWorkspace } from "../types";
import { apiMessage } from "../web/format";
import { useTemplateCatalogue } from "../web/workspace/data";
import { TemplatePicker } from "./TemplatePicker";
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
 * the template and whether people are added one by one. Opened from the **+** menu and from an organization's page.
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
  const [busy, setBusy] = useState(false);
  const [templates] = useTemplateCatalogue(accessToken, open);

  // Each time the dialog opens: the organization it was opened from, else the first one that allows it; nothing typed yet.
  useEffect(() => {
    if (!open) return;
    setOrganizationId(choices.some((organization) => organization.id === defaultOrganizationId) ? defaultOrganizationId! : choices[0]?.id ?? "");
    setName(""); setTemplateId("general"); setRestricted(false);
  }, [open]);

  async function create() {
    if (!organizationId || !name.trim()) return;
    setBusy(true);
    try {
      const workspace = await createSharedWorkspace(accessToken, organizationId, name.trim(), templates.length > 1 ? templateId : undefined, restricted);
      showToast("success", `${workspace.workspace.name} was created.`);
      onCreated(workspace);
      onClose();
    } catch (requestError) { showToast("error", apiMessage(requestError)); } finally { setBusy(false); }
  }

  if (!choices.length) {
    return <Dialog description="Only an organization's owners and administrators can create its workspaces. Ask one of them, or create an organization of your own." footer={<DialogCancel label="Close" onClick={onClose} />} onClose={onClose} open={open} title="New workspace" />;
  }
  return <Dialog
    description="A bounded memory space for a repository, a system or an initiative."
    footer={<><DialogCancel onClick={onClose} /><Button busy={busy} disabled={!organizationId || !name.trim()} type="submit" variant="main">Create workspace</Button></>}
    onClose={onClose}
    onSubmit={() => void create()}
    open={open}
    size="md"
    title="New workspace"
  >
    <Field label="Organization">{(control) => <Dropdown disabled={busy} id={control.id} onValueChange={setOrganizationId} options={choices.map((organization) => ({ label: organization.name, value: organization.id }))} value={organizationId} />}</Field>
    <Field label="Workspace name" required><Input autoFocus disabled={busy} onChange={(event) => setName(event.target.value)} placeholder="Payments platform" required value={name} /></Field>
    <TemplatePicker disabled={busy} name="new-workspace-template" onChange={setTemplateId} templates={templates} value={templateId} />
    <Switch checked={restricted} disabled={busy} label="Restricted: add people one by one, not the whole organization" onCheckedChange={setRestricted} />
  </Dialog>;
}
