import { useEffect, useState } from "react";
import {
  IconEyeCheck as EyeCheck,
  IconFileAlert as FileAlert,
  IconNotebook as Notebook,
  IconRefresh as Refresh,
  IconStethoscope as Stethoscope,
} from "@tabler/icons-react";
import { bringBackSharedReviewItem, getSharedReviewQueue, snoozeSharedReviewItem } from "../lib/sharedApi";
import type { ReviewItem, ReviewItemKind, ReviewQueue } from "../types";
import { useLiveRefresh } from "../lib/liveEvents";
import { queryClient, workspaceKeys } from "../lib/queryClient";
import { Button } from "./ui/button";
import { FeedbackNotice } from "./ui/feedback-notice";
import { Dropdown } from "./ui/dropdown";
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

/** How long an item can be put aside, in days; the item comes back at the start of that day. */
export const SNOOZE_CHOICES = [
  { days: 1, label: "Until tomorrow" },
  { days: 7, label: "For a week" },
  { days: 30, label: "For a month" },
];

/** The start of the day `days` from now, in the person's own time zone, as an instant. */
export function snoozeUntil(days: number, now = new Date()): string {
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + days, 0, 0, 0).toISOString();
}

function since(value: string | null) {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : `Waiting since ${date.toLocaleDateString(undefined, { dateStyle: "medium" })}`;
}

function backOn(value: string | null | undefined) {
  const date = value ? new Date(value) : null;
  return date && !Number.isNaN(date.getTime()) ? `Back on ${date.toLocaleDateString(undefined, { dateStyle: "medium" })}` : "Snoozed";
}

/**
 * Everything in the workspace that waits for a person, in one list: files
 * marked for review, memory cards whose evidence changed and open health
 * findings. Each item opens where it is resolved and leaves the list once it is.
 * An item can be snoozed (V0.3.10): it leaves the person's own list and the count
 * on the Review tab until the day they chose, and can be brought back at once.
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
  const [loadError, setLoadError] = useState<string | null>(null);

  async function load() {
    setIsRefreshing(true);
    setLoadError(null);
    try {
      const next = await getSharedReviewQueue(accessToken, workspaceId);
      setQueue(next);
      // The Review tab's count reads the same answer.
      queryClient.setQueryData(workspaceKeys.review(workspaceId), next);
    } catch (error) {
      setLoadError(apiMessage(error, "The review queue could not be read."));
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

  async function snooze(item: ReviewItem, days: number) {
    try {
      await snoozeSharedReviewItem(accessToken, workspaceId, item, snoozeUntil(days));
      showToast("success", `${item.title} is snoozed. It comes back on ${new Date(snoozeUntil(days)).toLocaleDateString(undefined, { dateStyle: "medium" })}.`);
      await load();
    } catch (error) {
      showToast("error", apiMessage(error, "That item could not be snoozed."));
    }
  }

  async function bringBack(item: ReviewItem) {
    try {
      await bringBackSharedReviewItem(accessToken, workspaceId, item);
      await load();
    } catch (error) {
      showToast("error", apiMessage(error, "That item could not be brought back."));
    }
  }

  if (!queue) {
    return <section className="rm-health" aria-busy={isRefreshing}>{loadError ? <FeedbackNotice action="Try again" detail={loadError} kind="error" onAction={() => void load()} title="Review queue unavailable" /> : <FeedbackNotice kind="loading" title="Reading the review queue" />}</section>;
  }

  const count = queue.items.length;
  const snoozed = queue.snoozed ?? [];
  return <section className="rm-health" aria-busy={isRefreshing}>
    {loadError ? <FeedbackNotice action="Try again" detail={loadError} kind="error" onAction={() => void load()} title="Review queue could not be refreshed" /> : null}
    <div className="rm-map-toolbar">
      <span>{count ? `${count} ${count === 1 ? "item" : "items"} waiting · ${queue.evidence_count} ${queue.evidence_count === 1 ? "file" : "files"} · ${queue.memory_card_count} memory ${queue.memory_card_count === 1 ? "card" : "cards"} · ${queue.finding_count} ${queue.finding_count === 1 ? "finding" : "findings"}` : "Nothing is waiting"}{snoozed.length ? ` · ${snoozed.length} snoozed` : ""}</span>
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
            <Dropdown aria-label={`Snooze ${item.title}`} className="rm-review-snooze" onValueChange={(days) => void snooze(item, Number(days))} options={SNOOZE_CHOICES.map((choice) => ({ label: choice.label, value: String(choice.days) }))} placeholder="Snooze" value="" />
            {cannotResolve ? <span className="rm-map-muted">{item.resolver === "admin" ? "Only an owner or administrator can resolve this." : "You can read this; someone who can edit the workspace resolves it."}</span> : null}
          </div>
        </li>;
      })}
    </ul> : <div className="shared-empty-state"><EyeCheck size={26} /><strong>Nothing is waiting for review</strong><span>Files marked <em>needs review</em>, memory cards whose evidence changed and open health findings appear here, and leave as they are resolved.</span></div>}
    {snoozed.length ? <section aria-label="Snoozed items" className="rm-review-snoozed">
      <h3>Snoozed ({snoozed.length})</h3>
      <p className="rm-map-muted">Only your own list: they come back by themselves on the day shown.</p>
      <ul className="rm-health-list">
        {snoozed.map((item) => <li className="rm-health-card" key={`${item.kind}-${item.id}`}>
          <div className="rm-health-card-head">
            <span className="rm-health-kind">{KINDS[item.kind].icon}{KINDS[item.kind].label} · {backOn(item.snoozed_until)}</span>
            <h3>{item.title}</h3>
          </div>
          <div className="rm-health-actions">
            <Button onClick={() => void bringBack(item)} type="button" variant="secondary">Bring back</Button>
          </div>
        </li>)}
      </ul>
    </section> : null}
  </section>;
}
