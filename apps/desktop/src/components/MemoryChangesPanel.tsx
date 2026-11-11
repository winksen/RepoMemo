import { useEffect, useState } from "react";
import { IconGitCompare as Compare } from "@tabler/icons-react";
import { getSharedMemoryChanges } from "../lib/sharedApi";
import type { MemoryChange, MemoryChanges } from "../types";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { apiMessage } from "../web/format";

/**
 * What changed behind a card's "needs review" flag (V0.6.9): the earlier and current text of the cited file when an earlier
 * version is kept, otherwise the reason it cannot be shown. A reviewer can open the file, or remove the citation that no
 * longer holds, without leaving the card.
 */
export function MemoryChangesPanel({ accessToken, canWrite, cardId, isBusy, onOpenArtifact, onRemoveCitation, updatedAt }: { accessToken: string; canWrite: boolean; cardId: string; isBusy: boolean; onOpenArtifact: (artifactId: string) => void; onRemoveCitation: (artifactId: string | null) => void; updatedAt: string }) {
  const [changes, setChanges] = useState<MemoryChanges | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSharedMemoryChanges(accessToken, cardId).then((next) => { if (!cancelled) setChanges(next); }).catch((error) => { if (!cancelled) showToast("error", apiMessage(error, "The changes could not be read.")); });
    return () => { cancelled = true; };
  }, [accessToken, cardId, updatedAt]);

  if (!changes || !changes.changes.length) return null;
  return <section aria-labelledby={`changes-${cardId}`} className="shared-record-panel shared-review-panel">
    <PanelHeading description={`${changes.changes.length} ${changes.changes.length === 1 ? "change" : "changes"}`} icon={Compare} id={`changes-${cardId}`} title="What changed" />
    <ul className="rm-change-list">
      {changes.changes.map((change: MemoryChange, index) => <li key={`${change.kind}-${change.artifact_id ?? index}`}>
        <h3>{change.title || "A cited source"}</h3>
        <p>{change.summary}</p>
        {change.comparison && change.comparison.lines.length ? <div aria-label={`Changes to ${change.title}`} className="shared-version-diff" role="table">
          {change.comparison.lines.map((line, lineIndex) => <div className={`shared-diff-line ${line.kind}`} key={lineIndex} role="row">
            <span aria-hidden="true" className="shared-diff-mark">{line.kind === "added" ? "+" : line.kind === "removed" ? "−" : ""}</span>
            <code>{line.kind === "gap" ? `… ${line.text}` : line.text || " "}</code>
            {line.kind === "added" ? <span className="sr-only">added</span> : line.kind === "removed" ? <span className="sr-only">removed</span> : null}
          </div>)}
        </div> : null}
        {change.unavailable_reason ? <p className="shared-muted-copy">{change.unavailable_reason}</p> : null}
        <div className="shared-detail-actions">
          {change.artifact_id ? <Button onClick={() => onOpenArtifact(change.artifact_id!)} type="button" variant="secondary">Open the file</Button> : null}
          {canWrite ? <Button disabled={isBusy} onClick={() => onRemoveCitation(change.artifact_id)} type="button" variant="secondary">{change.artifact_id ? "Remove this citation" : "Remove the missing citation"}</Button> : null}
        </div>
      </li>)}
    </ul>
  </section>;
}
