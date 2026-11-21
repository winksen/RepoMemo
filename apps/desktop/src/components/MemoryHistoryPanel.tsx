import { useEffect, useState } from "react";
import { ClockCounterClockwise as History, CircleNotch as Loader } from "@phosphor-icons/react";
import { compareSharedMemoryRevisions, getSharedCitedSource, listSharedMemoryRevisions, restoreSharedMemoryRevision, SharedApiError } from "../lib/sharedApi";
import type { CitedSource, MemoryRevision, MemoryRevisionComparison } from "../types";
import { apiMessage } from "../web/format";
import { formatDateTimeValue } from "../lib/locale";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { Dialog, DialogCancel } from "./ui/dialog";
import { useConfirm } from "./ui/confirm-dialog";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";

/** How a historical citation stands against the file as it is now. A citation is never shown as current unless it is. */
const CURRENCY: Record<string, string> = {
  current: "the file is as cited",
  changed: "the file has changed since",
  gone: "the file no longer exists",
  unbound: "content at the time not recorded",
};

const when = formatDateTimeValue;

/**
 * The saved revisions of a memory card (V0.6.3): who changed the statement and its citations, when, a comparison with the
 * current revision, and a restore that adds a new revision rather than rewriting history. Shown once a card has more
 * than one revision.
 */
export function MemoryHistoryPanel({ accessToken, canWrite, cardId, onRestored, updatedAt }: { accessToken: string; canWrite: boolean; cardId: string; onRestored: () => void; updatedAt: string }) {
  const [revisions, setRevisions] = useState<MemoryRevision[] | null>(null);
  const [nextBefore, setNextBefore] = useState<number | null>(null);
  const [total, setTotal] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [comparison, setComparison] = useState<MemoryRevisionComparison | "loading" | null>(null);
  const [restoring, setRestoring] = useState<number | null>(null);
  const [inspecting, setInspecting] = useState<MemoryRevision | null>(null);
  const [source, setSource] = useState<CitedSource | { error: string } | "loading" | null>(null);
  const confirm = useConfirm();

  async function viewSource(revision: number, index: number) {
    setSource("loading");
    try {
      setSource(await getSharedCitedSource(accessToken, cardId, revision, index));
    } catch (error) {
      // A refusal is not the same as a source that is gone: say which it was.
      setSource({ error: error instanceof SharedApiError && (error.status === 403 || error.status === 404) ? "You no longer have access to this card, so its sources cannot be shown." : apiMessage(error, "The cited text could not be read.") });
    }
  }

  useEffect(() => {
    let cancelled = false;
    listSharedMemoryRevisions(accessToken, cardId).then((page) => { if (!cancelled) { setRevisions(page.items); setNextBefore(page.nextBefore); setTotal(page.total); } }).catch((error) => { if (!cancelled) showToast("error", apiMessage(error, "The history could not be read.")); });
    return () => { cancelled = true; };
  }, [accessToken, cardId, updatedAt]);

  async function showEarlier() {
    if (nextBefore === null) return;
    setLoadingMore(true);
    try {
      const page = await listSharedMemoryRevisions(accessToken, cardId, nextBefore);
      setRevisions((kept) => [...(kept ?? []), ...page.items]);
      setNextBefore(page.nextBefore);
    } catch (error) {
      showToast("error", apiMessage(error, "Earlier revisions could not be read."));
    } finally {
      setLoadingMore(false);
    }
  }

  async function compare(from: number, to: number) {
    setComparison("loading");
    try {
      setComparison(await compareSharedMemoryRevisions(accessToken, cardId, from, to));
    } catch (error) {
      setComparison(null);
      showToast("error", apiMessage(error, "The revisions could not be compared."));
    }
  }

  async function restore(revision: number) {
    const next = (revisions?.[0]?.revision ?? 0) + 1;
    const agreed = await confirm({ action: "Restore", description: `Revision ${revision} becomes revision ${next}. Revision ${revision} and every later revision stay in the history. A cited passage or file that is gone cannot come back and is named afterwards.`, title: `Restore revision ${revision}?` });
    if (!agreed) return;
    setRestoring(revision);
    try {
      const result = await restoreSharedMemoryRevision(accessToken, cardId, revision);
      showToast("success", result.skipped_citations.length ? `Restored as revision ${result.revision}. ${result.skipped_citations.length} cited source${result.skipped_citations.length === 1 ? " is" : "s are"} gone and was left out: ${result.skipped_citations.map((citation) => citation.title || citation.target_id).join(", ")}.` : `Restored as revision ${result.revision}.`);
      onRestored();
    } catch (error) {
      showToast("error", apiMessage(error, "That revision could not be restored."));
    } finally {
      setRestoring(null);
    }
  }

  if (!revisions || total < 2) return null;
  const current = revisions[0];
  return <section className="shared-record-panel shared-versions-panel" aria-label="Revision history">
    {comparison ? <Dialog size="lg" description={comparison === "loading" ? "Comparing…" : `Revision ${comparison.from_revision} to revision ${comparison.to_revision}`} footer={<DialogCancel label="Close" onClick={() => setComparison(null)} />} onClose={() => setComparison(null)} open title="What changed">
      {comparison === "loading" ? <p className="shared-muted-copy"><Loader className="spin" size={ICON_SIZE.sm} /> Loading…</p> : <>
        {comparison.from_title !== comparison.to_title ? <p><strong>Title:</strong> {comparison.from_title} → {comparison.to_title}</p> : null}
        {comparison.body.lines.length ? <div aria-label="Changes to the statement" className="shared-version-diff" role="table">
          {comparison.body.lines.map((line, index) => <div className={`shared-diff-line ${line.kind}`} key={index} role="row">
            <span aria-hidden="true" className="shared-diff-mark">{line.kind === "added" ? "+" : line.kind === "removed" ? "−" : ""}</span>
            <code>{line.kind === "gap" ? `… ${line.text}` : line.text || " "}</code>
            {line.kind === "added" ? <span className="sr-only">added</span> : line.kind === "removed" ? <span className="sr-only">removed</span> : null}
          </div>)}
        </div> : <p className="shared-muted-copy">The statement is the same.</p>}
        {comparison.citations_added.length ? <p><strong>Citations added:</strong> {comparison.citations_added.map((citation) => citation.title || citation.target_id).join(", ")}</p> : null}
        {comparison.citations_removed.length ? <p><strong>Citations removed:</strong> {comparison.citations_removed.map((citation) => citation.title || citation.target_id).join(", ")}</p> : null}
        {!comparison.citations_added.length && !comparison.citations_removed.length ? <p className="shared-muted-copy">The citations are the same.</p> : null}
      </>}
    </Dialog> : null}
    {inspecting ? <Dialog size="lg" description={`What revision ${inspecting.revision} cited, against the files as they are now`} footer={<DialogCancel label="Close" onClick={() => { setInspecting(null); setSource(null); }} />} onClose={() => { setInspecting(null); setSource(null); }} open title={`Sources of revision ${inspecting.revision}`}>
      {inspecting.citations.length ? <ul className="shared-version-list">
        {inspecting.citations.map((citation, index) => <li key={`${citation.target_id}-${index}`}>
          <div><strong>{citation.title || citation.path || "A cited source"}</strong><small>{CURRENCY[citation.currency] ?? "state unknown"}{citation.content_hash ? ` · content ${citation.content_hash.slice(0, 12)}` : ""}</small></div>
          <div className="shared-version-actions"><Button onClick={() => void viewSource(inspecting.revision, index)} type="button" variant="secondary">View cited text</Button></div>
        </li>)}
      </ul> : <p className="shared-muted-copy">This revision cited nothing.</p>}
      {source === "loading" ? <p className="shared-muted-copy"><Loader className="spin" size={ICON_SIZE.sm} /> Loading…</p> : null}
      {source && source !== "loading" && "error" in source ? <p role="alert">{source.error}</p> : null}
      {source && source !== "loading" && "state" in source ? <div><p><strong>{source.title}</strong> · {source.state.replace("_", " ")}</p><p className="shared-muted-copy">{source.note}</p>{source.text ? <pre className="shared-version-text">{source.text}</pre> : null}{source.truncated ? <p className="shared-muted-copy">Only the start is shown.</p> : null}</div> : null}
    </Dialog> : null}
    <PanelHeading description={<>{revisions.length === total ? `${total} revisions` : `${revisions.length} of ${total} revisions`}</>} icon={History} title="Revision history" />
    <ul className="shared-version-list">
      {revisions.map((revision) => <li key={revision.revision}>
        <div>
          <strong>Revision {revision.revision}{revision.revision === current.revision ? " · current" : ""}{revision.restored_from ? ` · restored from revision ${revision.restored_from}` : ""}</strong>
          <small>{revision.author ? `Saved by ${revision.author.display_name} · ` : revision.author_label ? `Saved by ${revision.author_label} (on the server it was imported from) · ` : "Author not recorded · "}{when(revision.created_at)} · {revision.citations.length} {revision.citations.length === 1 ? "citation" : "citations"}</small>
        </div>
        <div className="shared-version-actions">
          {revision.citations.length ? <Button onClick={() => { setSource(null); setInspecting(revision); }} type="button" variant="secondary">Sources</Button> : null}
          {revision.revision > 1 ? <Button onClick={() => void compare(revision.revision - 1, revision.revision)} type="button" variant="secondary">What changed</Button> : null}
          {revision.revision !== current.revision ? <Button onClick={() => void compare(revision.revision, current.revision)} type="button" variant="secondary">Compare with current</Button> : null}
          {canWrite && revision.revision !== current.revision ? <Button aria-label={`Restore revision ${revision.revision}`} busy={restoring === revision.revision} disabled={restoring !== null} onClick={() => void restore(revision.revision)} type="button" variant="secondary">Restore</Button> : null}
        </div>
      </li>)}
    </ul>
    {nextBefore !== null ? <div><Button busy={loadingMore} onClick={() => void showEarlier()} type="button" variant="secondary">Show earlier revisions</Button></div> : null}
  </section>;
}
