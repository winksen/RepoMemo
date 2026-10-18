import { useEffect, useState } from "react";
import {
  IconEyeCheck as EyeCheck,
  IconFileAlert as FileAlert,
  IconNotebook as Notebook,
  IconRefresh as Refresh,
  IconStethoscope as Stethoscope,
} from "@tabler/icons-react";
import { getSharedReviewQueue } from "../lib/sharedApi";
import type { ReviewItem, ReviewItemKind, ReviewQueue } from "../types";
import { useLiveRefresh } from "../lib/liveEvents";
import { Button } from "./ui/button";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

const KINDS: Record<ReviewItemKind, { label: string; open: string; icon: JSX.Element }> = {
  evidence: { label: "File marked for review", open: "Open file", icon: <FileAlert size={ICON_SIZE.md} /> },
  memory_card: { label: "Memory card gone stale", open: "Open card", icon: <Notebook size={ICON_SIZE.md} /> },
  health_finding: { label: "Health finding", open: "Open in Health", icon: <Stethoscope size={ICON_SIZE.md} /> },
};

/** Activity that can add to or clear the queue. */
const REVIEW_ACTIONS = /^(evidence_|artifact_|memory_|repository_|workspace_health_|folder_)/;

function since(value: string | null) {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : `Waiting since ${date.toLocaleDateString(undefined, { dateStyle: "medium" })}`;
}

/**
 * Everything in the workspace that waits for a person, in one list: files
 * marked for review, memory cards whose evidence changed and open health
 * findings. Each item opens where it is resolved and leaves the list once it is.
 */
export function ReviewQueuePanel({
  accessToken,
  canWrite,
  isWorkspaceAdmin,
  onOpenArtifact,
  onOpenHealth,
  onOpenMemoryCard,
  workspaceId,
}: {
  accessToken: string;
  canWrite: boolean;
  isWorkspaceAdmin: boolean;
  onOpenArtifact: (artifactId: string) => void;
  onOpenHealth: () => void;
  onOpenMemoryCard: (cardId: string) => void;
  workspaceId: string;
}) {
  const [queue, setQueue] = useState<ReviewQueue | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  async function load() {
    setIsRefreshing(true);
    try {
      setQueue(await getSharedReviewQueue(accessToken, workspaceId));
    } catch (error) {
      showToast("error", apiMessage(error, "The review queue could not be read."));
    } finally {
      setIsRefreshing(false);
    }
  }

  useEffect(() => { setQueue(null); void load(); }, [accessToken, workspaceId]);
  useLiveRefresh(() => void load(), REVIEW_ACTIONS);

  function open(item: ReviewItem) {
    if (item.kind === "memory_card") onOpenMemoryCard(item.id);
    else if (item.kind === "health_finding") onOpenHealth();
    else if (item.artifact_id) onOpenArtifact(item.artifact_id);
  }

  if (!queue) {
    return <section className="rm-health" aria-busy="true"><p className="shared-muted-copy">Reading the review queue…</p></section>;
  }

  const count = queue.items.length;
  return <section className="rm-health" aria-busy={isRefreshing}>
    <div className="rm-map-toolbar">
      <span>{count ? `${count} ${count === 1 ? "item" : "items"} waiting · ${queue.evidence_count} ${queue.evidence_count === 1 ? "file" : "files"} · ${queue.memory_card_count} memory ${queue.memory_card_count === 1 ? "card" : "cards"} · ${queue.finding_count} ${queue.finding_count === 1 ? "finding" : "findings"}` : "Nothing is waiting"}</span>
      <Button disabled={isRefreshing} onClick={() => void load()} type="button" variant="secondary"><Refresh size={ICON_SIZE.sm} /> Refresh</Button>
    </div>
    {count ? <ul aria-label="Review queue" className="rm-health-list">
      {queue.items.map((item) => {
        const kind = KINDS[item.kind];
        const cannotResolve = item.resolver === "admin" ? !isWorkspaceAdmin : !canWrite;
        return <li className="rm-health-card" key={`${item.kind}-${item.id}`}>
          <div className="rm-health-card-head">
            <span className="rm-health-kind">{kind.icon}{kind.label}{since(item.since) ? ` · ${since(item.since)}` : ""}</span>
            <h3>{item.title}</h3>
            <p>{item.reason}</p>
            {item.detail ? <p className="rm-map-muted">{item.detail}</p> : null}
          </div>
          <div className="rm-health-actions">
            <Button onClick={() => open(item)} type="button" variant="secondary">{kind.open}</Button>
            {cannotResolve ? <span className="rm-map-muted">{item.resolver === "admin" ? "Only an owner or administrator can resolve this." : "You can read this; someone who can edit the workspace resolves it."}</span> : null}
          </div>
        </li>;
      })}
    </ul> : <div className="shared-empty-state"><EyeCheck size={26} /><strong>Nothing is waiting for review</strong><span>Files marked <em>needs review</em>, memory cards whose evidence changed and open health findings appear here, and leave as they are resolved.</span></div>}
  </section>;
}
