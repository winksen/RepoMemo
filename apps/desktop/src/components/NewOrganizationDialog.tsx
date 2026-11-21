import { useEffect, useState } from "react";
import { createSharedOrganization, createSharedWorkspace } from "../lib/sharedApi";
import type { Organization, SharedWorkspace } from "../types";
import { apiMessage } from "../web/format";
import { useTemplateCatalogue } from "../web/workspace/data";
import { workspaceColorProps } from "../web/workspaceColors";
import { WorkspaceIcon } from "../web/workspaceIcons";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { Switch } from "./ui/switch";
import { showToast } from "./ui/toast";
import { DEFAULT_WORKSPACE_SETUP, type WorkspaceSetup, WorkspaceSetupFields, applyWorkspaceLook, lookWarning } from "./workspaceSetup";

/**
 * *New organization*, laid out like *New workspace*: the name, and optionally a first workspace with its name, icon, color,
 * template and access, so a new organization does not start empty. The person who creates it is its owner.
 */
export function NewOrganizationDialog({ accessToken, onClose, onCreated, open }: {
  accessToken: string;
  onClose: () => void;
  /** Called with the new organization and, when one was made, its first workspace. */
  onCreated: (organization: Organization, workspace: SharedWorkspace | null) => void;
  open: boolean;
}) {
  const [name, setName] = useState("");
  const [withWorkspace, setWithWorkspace] = useState(true);
  const [workspaceName, setWorkspaceName] = useState("General");
  const [setup, setSetup] = useState<WorkspaceSetup>(DEFAULT_WORKSPACE_SETUP);
  const [busy, setBusy] = useState(false);
  const [templates] = useTemplateCatalogue(accessToken, open && withWorkspace);

  useEffect(() => {
    if (!open) return;
    setName(""); setWithWorkspace(true); setWorkspaceName("General"); setSetup(DEFAULT_WORKSPACE_SETUP);
  }, [open]);

  const canCreate = Boolean(name.trim()) && (!withWorkspace || Boolean(workspaceName.trim()));

  async function create() {
    if (!canCreate) return;
    setBusy(true);
    let organization: Organization;
    try {
      organization = await createSharedOrganization(accessToken, name.trim());
    } catch (requestError) { showToast("error", apiMessage(requestError)); setBusy(false); return; }
    let workspace: SharedWorkspace | null = null;
    if (withWorkspace) {
      try {
        const created = await createSharedWorkspace(accessToken, organization.id, workspaceName.trim(), templates.length > 1 ? setup.templateId : undefined, setup.restricted);
        const applied = await applyWorkspaceLook(accessToken, created, setup);
        workspace = applied.workspace;
        if (applied.failed.length) showToast("warning", lookWarning(applied.failed));
      } catch (requestError) {
        // The organization exists; only its first workspace is missing.
        showToast("warning", `${organization.name} was created, but its first workspace could not be: ${apiMessage(requestError)} Create it from the organization's page.`);
      }
    }
    showToast("success", `${organization.name} was created.`);
    onCreated(organization, workspace);
    onClose();
    setBusy(false);
  }

  return <Dialog
    className="shared-new-workspace-dialog"
    description="A separate team boundary, with its own members, templates and workspaces. You will be its owner."
    footer={<><DialogCancel onClick={onClose} /><Button busy={busy} disabled={!canCreate} type="submit" variant="main">Create organization</Button></>}
    onClose={onClose}
    onSubmit={() => void create()}
    open={open}
    size="lg"
    title="New organization"
  >
    <div className="shared-new-workspace-row">
      <Field label="Organization name" required><Input autoFocus disabled={busy} onChange={(event) => setName(event.target.value)} placeholder="Engineering" required value={name} /></Field>
      <Field label="First workspace name" required={withWorkspace}><Input disabled={busy || !withWorkspace} onChange={(event) => setWorkspaceName(event.target.value)} placeholder="General" value={withWorkspace ? workspaceName : ""} /></Field>
    </div>
    <p className="shared-new-workspace-as" aria-live="polite">
      {withWorkspace ? <span className="shared-rail-chip" {...workspaceColorProps(setup.color)}><WorkspaceIcon size={20} weight="fill" workspace={{ settings: { icon: setup.icon } }} /></span> : null}
      <span>Organization will be created as <strong>{name.trim() || "Organization name"}</strong>{withWorkspace ? <>, with the workspace <strong>{name.trim() || "Organization name"} / {workspaceName.trim() || "Workspace name"}</strong></> : null}</span>
    </p>
    <div className="shared-access-choice">
      <Switch aria-label="Create a first workspace" checked={withWorkspace} disabled={busy} onCheckedChange={setWithWorkspace} />
      <div>
        <strong>{withWorkspace ? "Start with a first workspace" : "Start empty"}</strong>
        <span>{withWorkspace ? "Choose how it looks and what it starts with below. You can add more workspaces later." : "The organization is created with no workspace; add one from its page."}</span>
      </div>
    </div>
    {withWorkspace ? <WorkspaceSetupFields busy={busy} onChange={setSetup} organizationName={name.trim()} setup={setup} templates={templates} /> : null}
  </Dialog>;
}
