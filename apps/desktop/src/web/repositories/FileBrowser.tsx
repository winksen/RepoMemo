import { useEffect, useMemo, useState } from "react";
import { listSharedRepositoryFiles } from "../../lib/sharedApi";
import type { RepoFile, RepoSource } from "../../types";
import { apiMessage, lastChangeLabel } from "../format";
import { SearchField } from "../../components/ui/search-field";
import { showToast } from "../../components/ui/toast";
import { FILE_LIST_LIMIT } from "./shared";

export function RepositoryFiles({
  accessToken,
  onOpenArtifact,
  repository,
}: {
  accessToken: string;
  onOpenArtifact: (artifactId: string) => void;
  repository: RepoSource;
}) {
  const [files, setFiles] = useState<RepoFile[] | null>(null);
  const [filter, setFilter] = useState("");
  const commitSha = repository.last_synced_commit?.sha;

  useEffect(() => {
    let cancelled = false;
    listSharedRepositoryFiles(accessToken, repository.id)
      .then((next) => { if (!cancelled) setFiles(next); })
      .catch((error) => showToast("error", apiMessage(error, "The files could not be loaded.")));
    return () => { cancelled = true; };
  }, [accessToken, repository.id, commitSha, repository.indexed_file_count]);

  const matching = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    return (files ?? []).filter((file) => !needle || file.path.toLowerCase().includes(needle));
  }, [files, filter]);

  if (!files) return <p className="shared-muted-copy">Loading files…</p>;
  return <div className="rm-repo-files">
    <SearchField aria-label={`Filter files of ${repository.name}`} onValueChange={setFilter} placeholder="Filter by path" value={filter} />
    <ul>
      {matching.slice(0, FILE_LIST_LIMIT).map((file) => <li key={file.artifact_id}>
        <button className="rm-map-link" onClick={() => onOpenArtifact(file.artifact_id)} type="button">{file.path}</button>
        {file.last_author ? <span className="rm-map-muted"> · {lastChangeLabel(file.last_author, file.last_changed_at)}</span> : null}
        <span className="rm-map-muted">{file.language ?? ""}</span>
        {file.indexed ? null : <span className="rm-map-muted" title={file.index_failure ?? undefined}>{file.index_failure ? "Not indexed" : "Indexing…"}</span>}
      </li>)}
    </ul>
    <span className="rm-map-muted">{matching.length > FILE_LIST_LIMIT ? `Showing ${FILE_LIST_LIMIT} of ${matching.length.toLocaleString()} files. Filter to narrow the list.` : `${matching.length.toLocaleString()} ${matching.length === 1 ? "file" : "files"}`}</span>
  </div>;
}
