import { useState } from "react";
import { IconFolderCode as FolderCode, IconPlus as Plus, IconSettings as Settings } from "@tabler/icons-react";
import { cancelSharedJob, syncSharedRepository } from "../../lib/sharedApi";
import type { RepoSource } from "../../types";
import { apiMessage } from "../format";
import { Button } from "../../components/ui/button";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { useRepositories } from "./useRepositories";
import { RepositoryCard } from "./RepositoryCard";

/** The Repositories section: each linked repository's sync state, its last
 *  sync, and the files it keeps in step with the repository. Links are added
 *  and changed in workspace Settings. */
export function RepositoriesPanel({
  accessToken,
  canManage,
  canSync,
  onChanged,
  onOpenArtifact,
  onOpenSettings,
  workspaceId,
}: {
  accessToken: string;
  canManage: boolean;
  canSync: boolean;
  onChanged: () => void;
  onOpenArtifact: (artifactId: string) => void;
  onOpenSettings: () => void;
  workspaceId: string;
}) {
  const { repositories, load, track } = useRepositories(accessToken, workspaceId, onChanged);
  const [openFilesId, setOpenFilesId] = useState<string | null>(null);

  async function sync(repository: RepoSource) {
    try {
      const { job } = await syncSharedRepository(accessToken, repository.id);
      track(repository.id, job?.id);
      await load();
    } catch (error) {
      showToast("error", apiMessage(error, "The sync could not be started."));
    }
  }

  async function stop(repository: RepoSource) {
    if (!repository.active_job) return;
    try {
      await cancelSharedJob(accessToken, repository.active_job.id);
      await load();
    } catch (error) {
      showToast("error", apiMessage(error, "The sync could not be stopped."));
    }
  }

  if (!repositories) {
    return <section className="rm-repos" aria-busy="true"><p className="shared-muted-copy">Loading repositories…</p></section>;
  }

  const count = repositories.length;
  return <section className="rm-repos">
    <div className="rm-map-toolbar">
      <span>{count ? `${count} ${count === 1 ? "repository" : "repositories"}` : "No repositories linked"}</span>
      {canManage ? <Button onClick={onOpenSettings} type="button" variant="secondary"><Settings size={ICON_SIZE.sm} /> Manage links in Settings</Button> : null}
    </div>

    {count ? <ul className="rm-repo-list">
      {repositories.map((repository) => <RepositoryCard
        accessToken={accessToken}
        canSync={canSync}
        filesOpen={openFilesId === repository.id}
        key={repository.id}
        onOpenArtifact={onOpenArtifact}
        onStop={() => void stop(repository)}
        onSync={() => void sync(repository)}
        onToggleFiles={() => setOpenFilesId((current) => current === repository.id ? null : repository.id)}
        repository={repository}
      />)}
    </ul> : <div className="shared-empty-state">
      <FolderCode size={25} />
      <strong>No repositories yet</strong>
      <span>{canManage ? "Add a repository link in this workspace's Settings. Its committed files are indexed and kept in step with each sync." : "Owners and administrators can link a git repository to this workspace in Settings."}</span>
      {canManage ? <Button onClick={onOpenSettings} type="button" variant="main"><Plus size={ICON_SIZE.sm} /> Add a repository link</Button> : null}
    </div>}
  </section>;
}
