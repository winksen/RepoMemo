import { useEffect, useMemo, useState } from "react";
import { listSharedRepositoryFiles } from "../../lib/sharedApi";
import type { RepoFile, RepoSource } from "../../types";
import { apiMessage, lastChangeLabel } from "../format";
import { LoadingState } from "../../components/ui/loading-state";
import { SearchField } from "../../components/ui/search-field";
import { showToast } from "../../components/ui/toast";
import { Pagination } from "../../components/ui/pagination";
import { usePagedList } from "../../lib/usePagedList";
import { repositoryKeys } from "../../lib/queryClient";
import { useServerState } from "../../lib/useServerState";

export function RepositoryFiles({
  accessToken,
  onOpenArtifact,
  repository,
}: {
  accessToken: string;
  onOpenArtifact: (artifactId: string) => void;
  repository: RepoSource;
}) {
  const [filter, setFilter] = useState("");
  // Read again when the repository's commit or its count of indexed files changes.
  const [files, , query] = useServerState<RepoFile[] | null>({
    key: repositoryKeys.files(repository.id, repository.last_synced_commit?.sha, repository.indexed_file_count),
    fetch: () => listSharedRepositoryFiles(accessToken, repository.id),
    initial: null,
  });
  useEffect(() => {
    if (query.error) showToast("error", apiMessage(query.error, "The files could not be loaded."));
  }, [query.error]);

  const matching = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    return (files ?? []).filter((file) => !needle || file.path.toLowerCase().includes(needle));
  }, [files, filter]);

  const paging = usePagedList(matching, filter);
  if (!files) return <LoadingState label="Loading files" />;
  return <div className="rm-repo-files">
    <SearchField aria-label={`Filter files of ${repository.name}`} onValueChange={setFilter} placeholder="Filter by path" value={filter} />
    <ul>
      {paging.rows.map((file) => <li key={file.artifact_id}>
        <button className="rm-map-link" onClick={() => onOpenArtifact(file.artifact_id)} type="button">{file.path}</button>
        {file.last_author ? <span className="rm-map-muted"> · {lastChangeLabel(file.last_author, file.last_changed_at)}</span> : null}
        <span className="rm-map-muted">{file.language ?? ""}</span>
        {file.indexed ? null : <span className="rm-map-muted" title={file.index_failure ?? undefined}>{file.index_failure ? "Not indexed" : "Indexing…"}</span>}
      </li>)}
    </ul>
    <Pagination {...paging.props} />
    <span className="rm-map-muted">{matching.length.toLocaleString()} {matching.length === 1 ? "file" : "files"}</span>
  </div>;
}
