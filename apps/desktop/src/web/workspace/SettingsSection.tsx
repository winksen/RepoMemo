import { type FormEvent, useEffect } from "react";
import { IconPencil as Pencil, IconSettings as Settings, IconTrash as Trash } from "@tabler/icons-react";
import { deleteSharedWorkspace, getSharedWorkspaceMetrics, updateSharedWorkspace } from "../../lib/sharedApi";
import { type SharedAiProviderSettings } from "../../types";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { AiPolicySettings } from "../../components/AiPolicySettings";
import { AiProviderForm } from "../../components/AiProviderForm";
import { ConnectorSettings } from "../../components/ConnectorsPanel";
import { RepositorySettings } from "../lazy";
import { INDEX_POLL_ATTEMPTS, INDEX_POLL_INTERVAL_MS } from "../constants";
import { apiMessage } from "../format";
import { useAiProviders, useMetrics } from "./data";
import { type WorkspacePage, sectionState } from "./page";

export function SettingsSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, canManageMembers, canManageWorkspace, isLive, isSubmitting, setIsSubmitting, setError, load, onWorkspaceUpdated, onWorkspaceDeleted } = page;
  const isSettingsView = page.section === "settings";
  const useS = sectionState(page.store, "settings");
  const [workspaceName, setWorkspaceName] = useS("workspaceName", workspace.workspace.name);
  const [aiProviders, setAiProviders] = useAiProviders(accessToken, workspaceId, canManageMembers);
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

  async function renameWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      const updated = await updateSharedWorkspace(accessToken, workspace.workspace.id, workspaceName);
      onWorkspaceUpdated(updated);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeWorkspace() {
    if (!window.confirm(`Delete ${workspace.workspace.name}? This permanently removes all shared evidence and memory in this workspace.`)) return;
    setIsSubmitting(true); setError(null);
    try {
      await deleteSharedWorkspace(accessToken, workspace.workspace.id);
      onWorkspaceDeleted(workspace.workspace.id);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function onProviderSaved(saved: SharedAiProviderSettings) {
    setAiProviders((current) => [saved, ...current.filter((entry) => entry.id !== saved.id)]);
    // Saving an image provider makes the server retry images that were waiting.
    void load();
  }

  return (
          <section className="shared-settings-section">
            {canManageMembers ? <>
              <div className="shared-settings-policy"><Settings size={18} /><div><strong>Administrative workspace controls</strong><span>Changes are enforced by the shared API. Administrators can manage people and integrations; owner-only workspace actions are explicitly marked.</span></div></div>
              <AiProviderForm accessToken={accessToken} onSaved={onProviderSaved} providers={aiProviders} purpose="text" workspaceId={workspace.workspace.id} />
              <AiProviderForm accessToken={accessToken} onSaved={onProviderSaved} providers={aiProviders} purpose="vision" workspaceId={workspace.workspace.id} />
              <AiProviderForm accessToken={accessToken} onSaved={onProviderSaved} providers={aiProviders} purpose="embedding" status={embeddingStatus} workspaceId={workspace.workspace.id} />
              <AiPolicySettings accessToken={accessToken} key={`ai-policy-${workspace.workspace.id}`} onSaved={() => void load()} workspaceId={workspace.workspace.id} />
              <RepositorySettings accessToken={accessToken} key={`repositories-${workspace.workspace.id}`} onChanged={() => void load()} workspaceId={workspace.workspace.id} />
              <ConnectorSettings accessToken={accessToken} key={`connectors-${workspace.workspace.id}`} onChanged={() => void load()} workspaceId={workspace.workspace.id} />
              {canManageWorkspace ? <section className="shared-settings-group">
                <div className="shared-panel-heading"><div><Settings size={18} /><h2>Workspace ownership</h2></div><span>owner only</span></div>
                <p className="shared-muted-copy">Rename this workspace or remove it permanently. Deletion removes its shared evidence, memory, and memberships.</p>
                <form className="shared-workspace-rename" onSubmit={renameWorkspace}><Input aria-label="Workspace name" onChange={(event) => setWorkspaceName(event.target.value)} required value={workspaceName} /><Button disabled={isSubmitting} type="submit" variant="secondary"><Pencil size={16} /> Rename</Button></form>
                <Button className="shared-danger-action" disabled={isSubmitting} onClick={() => void removeWorkspace()} type="button" variant="secondary"><Trash size={16} /> Delete workspace</Button>
              </section> : null}
            </> : <div className="shared-empty-state"><Settings size={25} /><strong>Administrative access required</strong><span>Only workspace administrators and the owner can open settings. Your current role can still browse shared evidence, memory, and tasks.</span></div>}
          </section>
  );
}
