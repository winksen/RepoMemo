import { useEffect, useState } from "react";
import { createSharedOrganization, createSharedWorkspace } from "../lib/sharedApi";
import type { Organization, SharedWorkspace } from "../types";
import { apiMessage } from "../web/format";
import { useTemplateCatalogue } from "../web/workspace/data";
import { DEFAULT_ORGANIZATION_ICON, OrganizationIcon } from "../web/organizationIcons";
import { workspaceColorProps } from "../web/workspaceColors";
import { WorkspaceIcon } from "../web/workspaceIcons";
import { OrganizationIconPicker } from "./WorkspaceLookPickers";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { Switch } from "./ui/switch";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";
import { DEFAULT_WORKSPACE_SETUP, type WorkspaceSetup, WorkspaceSetupFields, applyWorkspaceLook, lookWarning } from "./workspaceSetup";

/**
 * *New organization*, in two sections so that nothing about the workspace is mistaken for the organization: first the
 * organization (name, description, icon), then the choice of starting empty or with a first workspace, and below a line the
 * workspace's own choices (name, icon, color, template, access). The person who creates the organization is its owner.
 */
export function NewOrganizationDialog({ accessToken, onClose, onCreated, open }: {
  accessToken: string;
  onClose: () => void;
  /** Called with the new organization and, when one was made, its first workspace. */
  onCreated: (organization: Organization, workspace: SharedWorkspace | null) => void;
  open: boolean;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [icon, setIcon] = useState(DEFAULT_ORGANIZATION_ICON);
  const [withWorkspace, setWithWorkspace] = useState(true);
  const [workspaceName, setWorkspaceName] = useState("General");
  const [setup, setSetup] = useState<WorkspaceSetup>(DEFAULT_WORKSPACE_SETUP);
  const [busy, setBusy] = useState(false);
  const [templates] = useTemplateCatalogue(accessToken, open && withWorkspace);

  useEffect(() => {
    if (!open) return;
    setName(""); setDescription(""); setIcon(DEFAULT_ORGANIZATION_ICON);
    setWithWorkspace(true); setWorkspaceName("General"); setSetup(DEFAULT_WORKSPACE_SETUP);
  }, [open]);

  const canCreate = Boolean(name.trim()) && (!withWorkspace || Boolean(workspaceName.trim()));
  const organizationLabel = name.trim() || "Organization name";

  async function create() {
    if (!canCreate) return;
    setBusy(true);
    let organization: Organization;
    try {
      organization = await createSharedOrganization(accessToken, name.trim(), {
        description: description.trim() || undefined,
        icon: icon === DEFAULT_ORGANIZATION_ICON ? undefined : icon,
      });
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
    <section aria-label="Organization" className="shared-dialog-section">
      <div className="shared-new-workspace-row">
        <div className="shared-dialog-stack">
          <Field label="Organization name" required><Input autoFocus disabled={busy} onChange={(event) => setName(event.target.value)} placeholder="Engineering" required value={name} /></Field>
          <Field label="Description"><Textarea disabled={busy} maxLength={500} onChange={(event) => setDescription(event.target.value)} placeholder="What this organization is for" rows={3} value={description} /></Field>
        </div>
        <div className="shared-dialog-stack">
          <div className="shared-new-workspace-look-block"><strong>Icon</strong><OrganizationIconPicker disabled={busy} onChange={setIcon} value={icon} /></div>
          <p aria-live="polite" className="shared-new-workspace-as">
            <span className="shared-rail-chip"><OrganizationIcon organization={{ icon }} size={20} weight="fill" /></span>
            <span>Organization will be created as <strong>{organizationLabel}</strong></span>
          </p>
        </div>
      </div>
    </section>
    <div className="shared-access-choice">
      <Switch aria-label="Create a first workspace" checked={withWorkspace} disabled={busy} onCheckedChange={setWithWorkspace} />
      <div>
        <strong>{withWorkspace ? "Start with a first workspace" : "Start empty"}</strong>
        <span>{withWorkspace ? "Set it up below, under the line. You can add more workspaces later." : "The organization is created with no workspace; add one from its page."}</span>
      </div>
    </div>
    {withWorkspace ? <>
      <hr className="shared-dialog-divider" />
      <section aria-label="First workspace" className="shared-dialog-section">
        <h3 className="shared-dialog-section-title">First workspace</h3>
        <div className="shared-new-workspace-row">
          <Field label="Workspace name" required><Input disabled={busy} onChange={(event) => setWorkspaceName(event.target.value)} placeholder="General" value={workspaceName} /></Field>
          <p aria-live="polite" className="shared-new-workspace-as">
            <span className="shared-rail-chip" {...workspaceColorProps(setup.color)}><WorkspaceIcon size={20} weight="fill" workspace={{ settings: { icon: setup.icon } }} /></span>
            <span>Workspace will be created as <strong>{organizationLabel} / {workspaceName.trim() || "Workspace name"}</strong></span>
          </p>
        </div>
        <WorkspaceSetupFields busy={busy} onChange={setSetup} organizationName={name.trim()} setup={setup} templates={templates} />
      </section>
    </> : null}
  </Dialog>;
}
