import { useRef, useState } from "react";
import { useLiveRefresh } from "../../lib/liveEvents";
import { IconRefresh as Refresh } from "@tabler/icons-react";
import { getSharedKnowledgeMap } from "../../lib/sharedApi";
import type { KnowledgeMap } from "../../types";
import { Button } from "../../components/ui/button";
import { ICON_SIZE } from "../../components/ui/icon";
import { POLL_INTERVAL_MS, POLL_ATTEMPTS } from "./shared";
import { Pipeline } from "./Pipeline";
import { Coverage } from "./Coverage";
import { RelationGraph } from "./RelationGraph";
import { workspaceKeys } from "../../lib/queryClient";
import { useServerState } from "../../lib/useServerState";
import { useErrorToast } from "../../lib/useErrorToast";

/** Activity that can change the map. */
const MAP_ACTIONS = /^(evidence_|artifact_|memory_|repository_|folder_)/;

/** Whether files are still waiting to be indexed or passages to be embedded. */
function isIndexing(map: KnowledgeMap | null | undefined) {
  return Boolean(map && (map.pipeline.pending_count > 0 || (map.pipeline.embedded_count !== null && map.pipeline.embedded_count < map.pipeline.passage_count)));
}

/** The knowledge map: indexing and embedding progress, coverage per kind of
 *  file, and a graph of how files relate by meaning and through memory. */
export function KnowledgeMapPanel({
  accessToken,
  canConfigure,
  onOpenArtifact,
  onOpenEvidence,
  onOpenMemoryCard,
  onOpenSettings,
  workspaceId,
}: {
  accessToken: string;
  canConfigure: boolean;
  onOpenArtifact: (artifactId: string) => void;
  onOpenEvidence: () => void;
  onOpenMemoryCard: (cardId: string) => void;
  onOpenSettings: () => void;
  workspaceId: string;
}) {
  // Indexing and embedding run in the background; the map is read again while they do (up to
  // POLL_ATTEMPTS times), holding the previous picture instead of flashing.
  const attempts = useRef(0);
  const [map, , query] = useServerState<KnowledgeMap | null>({
    key: workspaceKeys.knowledgeMap(workspaceId),
    fetch: () => getSharedKnowledgeMap(accessToken, workspaceId),
    initial: null,
    staleTime: 0,
    refetchInterval: (current) => {
      if (!isIndexing(current)) { attempts.current = 0; return false; }
      return attempts.current++ < POLL_ATTEMPTS ? POLL_INTERVAL_MS : false;
    },
  });
  useErrorToast(query.error, "The knowledge map could not be loaded.");
  // Only a refresh the person (or a live event) asked for shows as busy, not the reads while indexing runs.
  const [isRefreshing, setIsRefreshing] = useState(false);
  async function load() {
    setIsRefreshing(true);
    try { await query.refetch(); } finally { setIsRefreshing(false); }
  }
  // New, changed and removed evidence and memory redraw the map as they happen.
  useLiveRefresh(() => void load(), MAP_ACTIONS);
  const inProgress = isIndexing(map);
  if (!map) {
    return <section className="rm-map" aria-busy="true"><p className="shared-muted-copy">Loading the knowledge map…</p></section>;
  }

  return <section className="rm-map" aria-busy={isRefreshing}>
    <div className="rm-map-toolbar">
      <span>{inProgress ? "Updating while indexing and embedding run…" : "Up to date"}</span>
      <Button disabled={isRefreshing} onClick={() => void load()} type="button" variant="secondary"><Refresh size={ICON_SIZE.sm} /> Refresh</Button>
    </div>
    <Pipeline canConfigure={canConfigure} map={map} onOpenEvidence={onOpenEvidence} onOpenSettings={onOpenSettings} />
    <Coverage map={map} />
    <RelationGraph canConfigure={canConfigure} map={map} onOpenArtifact={onOpenArtifact} onOpenMemoryCard={onOpenMemoryCard} onOpenSettings={onOpenSettings} />
  </section>;
}
