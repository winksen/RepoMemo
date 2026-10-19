import { useEffect, useState } from "react";
import { useLiveRefresh } from "../../lib/liveEvents";
import { IconRefresh as Refresh } from "@tabler/icons-react";
import { getSharedKnowledgeMap } from "../../lib/sharedApi";
import type { KnowledgeMap } from "../../types";
import { Button } from "../../components/ui/button";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";
import { POLL_INTERVAL_MS, POLL_ATTEMPTS } from "./shared";
import { Pipeline } from "./Pipeline";
import { Coverage } from "./Coverage";
import { RelationGraph } from "./RelationGraph";

/** Activity that can change the map. */
const MAP_ACTIONS = /^(evidence_|artifact_|memory_|repository_|folder_)/;

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
  const [map, setMap] = useState<KnowledgeMap | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  async function load() {
    setIsRefreshing(true);
    try {
      setMap(await getSharedKnowledgeMap(accessToken, workspaceId));
    } catch (error) {
      showToast("error", apiMessage(error, "The knowledge map could not be loaded."));
    } finally {
      setIsRefreshing(false);
    }
  }

  useEffect(() => { setMap(null); void load(); }, [accessToken, workspaceId]);
  // New, changed and removed evidence and memory redraw the map as they happen.
  useLiveRefresh(() => void load(), MAP_ACTIONS);

  // Indexing and embedding run in the background; keep the picture current
  // while they do, holding the previous render instead of flashing.
  const inProgress = Boolean(map && (map.pipeline.pending_count > 0 || (map.pipeline.embedded_count !== null && map.pipeline.embedded_count < map.pipeline.passage_count)));
  useEffect(() => {
    if (!inProgress) return;
    let attempts = 0;
    const timer = window.setInterval(() => {
      attempts += 1;
      void getSharedKnowledgeMap(accessToken, workspaceId).then(setMap).catch(() => undefined);
      if (attempts >= POLL_ATTEMPTS) window.clearInterval(timer);
    }, POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [accessToken, inProgress, workspaceId]);

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
