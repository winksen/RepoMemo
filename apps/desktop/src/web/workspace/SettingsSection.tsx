import { type FormEvent, useEffect } from "react";
import { PencilSimple as Pencil, Gear as Settings, Trash } from "@phosphor-icons/react";
import { deleteSharedWorkspace, getSharedWorkspaceMetrics, updateSharedWorkspace } from "../../lib/sharedApi";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { useConfirm } from "../../components/ui/confirm-dialog";
import { EmptyState } from "../../components/ui/empty-state";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { AiPolicySettings } from "../../components/AiPolicySettings";
import { WorkspaceCairnPanel } from "./WorkspaceCairnPanel";
import { ConnectorSettings } from "../../components/ConnectorsPanel";
import { WorkspaceExportSettings } from "../../components/WorkspaceExportSettings";
import { SearchFeedbackPanel } from "../../components/SearchFeedbackPanel";
import { WorkspaceTrashSettings } from "../../components/WorkspaceTrashSettings";
import { WorkspaceCustomization } from "../../components/WorkspaceCustomization";
import { TemplateSettings } from "../../components/TemplateSettings";
import { RepositorySettings } from "../lazy";
import { INDEX_POLL_ATTEMPTS, INDEX_POLL_INTERVAL_MS } from "../constants";
import { apiMessage } from "../format";
import { useUnsavedChanges } from "../../lib/unsavedChanges";
import { parseRoute } from "../router";
import { useMetrics } from "./data";
import { type WorkspacePage, sectionState } from "./page";
import { ICON_SIZE } from "../../components/ui/icon";
import { showToast } from "../../components/ui/toast";

export function SettingsSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, canManageMembers, canManageWorkspace, isLive, isSubmitting, setIsSubmitting, setError, load, onWorkspaceUpdated, onWorkspaceDeleted } = page;
  const isSettingsView = page.section === "settings";
  const useS = sectionState(page.store, "settings");
  const confirm = useConfirm();
  const [workspaceName, setWorkspaceName] = useS("workspaceName", workspace.workspace.name);
  const [workspaceMetrics, setWorkspaceMetrics] = useMetrics(accessToken, workspaceId);
  const embeddedChunks = workspaceMetrics?.embedded_chunk_count ?? null;
  const totalChunks = workspaceMetrics?.chunk_count ?? 0;
  const embeddingPending = embeddedChunks !== null && embeddedChunks < totalChunks;
  const embeddingStatus = embeddedChunks === null
    ? null
    : totalChunks === 0
      ? "No indexed passages yet. They are embedded as soon as files are indexed."
      : embeddingPending
        ? `${embeddedChunks.toLocaleString()} of ${totalChunks.toLocaleString()} passages embedded so far…`
        : `All ${totalChunks.toLocaleString()} indexed passages are searchable by meaning.`;
  // Embedding runs in the background on the server; keep the settings
  // progress current while it does, and stop after a while if it stalls.
  // Embedding job events refresh it while the live stream is open.
  useEffect(() => {
    if (!isSettingsView || !embeddingPending || isLive) return;
    let attempts = 0;
    const timer = window.setInterval(async () => {
      attempts += 1;
      try { setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id)); } catch { /* try again */ }
      if (attempts >= INDEX_POLL_ATTEMPTS) window.clearInterval(timer);
    }, INDEX_POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [accessToken, embeddingPending, isLive, isSettingsView, workspace.workspace.id]);

  useEffect(() => { setWorkspaceName(workspace.workspace.name); }, [workspace.workspace.name]);
  // The draft stays while the person moves between sections of this workspace.
  useUnsavedChanges(canManageWorkspace && workspaceName !== workspace.workspace.name, {
    keepsOn: (to) => { const route = parseRoute(to.split("?")[0]); return route.name === "workspace" && route.workspaceId === workspaceId; },
    subject: "the new workspace name",
  });

  async function renameWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      const updated = await updateSharedWorkspace(accessToken, workspace.workspace.id, workspaceName);
      onWorkspaceUpdated(updated);
      showToast("success", "Workspace renamed.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeWorkspace() {
    if (!(await confirm({ action: "Delete workspace", danger: true, description: "This permanently removes all shared evidence and memory in this workspace.", title: `Delete ${workspace.workspace.name}?` }))) return;
    setIsSubmitting(true); setError(null);
    try {
      await deleteSharedWorkspace(accessToken, workspace.workspace.id);
      onWorkspaceDeleted(workspace.workspace.id);
      showToast("success", "Workspace deleted.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
          <section className="shared-settings-section">
            {canManageMembers ? <>
              <WorkspaceCustomization accessToken={accessToken} onSaved={onWorkspaceUpdated} workspace={workspace.workspace}>
                <TemplateSettings accessToken={accessToken} embedded key={`template-${workspace.workspace.id}`} workspaceId={workspace.workspace.id} />
              </WorkspaceCustomization>
              <WorkspaceCairnPanel accessToken={accessToken} embeddingStatus={embeddingStatus} key={`cairn-${workspace.workspace.id}`} workspaceId={workspace.workspace.id} />
              <AiPolicySettings accessToken={accessToken} key={`ai-policy-${workspace.workspace.id}`} onSaved={() => void load()} workspaceId={workspace.workspace.id} />
              <RepositorySettings accessToken={accessToken} key={`repositories-${workspace.workspace.id}`} onChanged={() => void load()} workspaceId={workspace.workspace.id} />
              <ConnectorSettings accessToken={accessToken} key={`connectors-${workspace.workspace.id}`} onChanged={() => void load()} workspaceId={workspace.workspace.id} />
              <WorkspaceExportSettings accessToken={accessToken} key={`export-${workspace.workspace.id}`} workspaceId={workspace.workspace.id} />
              <SearchFeedbackPanel accessToken={accessToken} canAnswer={canManageMembers} key={`feedback-${workspace.workspace.id}`} workspaceId={workspace.workspace.id} />
              <WorkspaceTrashSettings accessToken={accessToken} canPurge={canManageWorkspace} key={`trash-${workspace.workspace.id}`} onRestored={() => void load()} workspaceId={workspace.workspace.id} />
              {canManageWorkspace ? <section className="shared-settings-group">
                <PanelHeading description="owner only" icon={Settings} title="Workspace ownership" />
                <p className="shared-muted-copy">Rename this workspace or remove it permanently. Deletion removes its shared evidence, memory, and memberships.</p>
                <form className="shared-workspace-rename" onSubmit={renameWorkspace}><Field label="Workspace name" required><Input onChange={(event) => setWorkspaceName(event.target.value)} value={workspaceName} /></Field><Button busy={isSubmitting} type="submit" variant="secondary"><Pencil size={ICON_SIZE.md} /> Rename</Button></form>
                <Button className="shared-danger-action" disabled={isSubmitting} onClick={() => void removeWorkspace()} variant="secondary"><Trash size={ICON_SIZE.md} /> Delete workspace</Button>
              </section> : null}
            </> : <EmptyState description="Only workspace administrators and the owner can open settings. Your current role can still browse shared evidence, memory, and tasks." icon={<Settings size={25} />} title="Administrative access required" />}
          </section>
  );
}
