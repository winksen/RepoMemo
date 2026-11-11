import { useEffect, useState } from "react";
import { IconRosetteDiscountCheck as Approved } from "@tabler/icons-react";
import { approveSharedMemoryCard, getSharedApprovalPolicy, getSharedMemoryApproval, setSharedApprovalPolicy, withdrawSharedMemoryApproval } from "../lib/sharedApi";
import type { MemoryApprovalStatus } from "../types";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { Switch } from "./ui/switch";
import { showToast } from "./ui/toast";
import { apiMessage } from "../web/format";

const when = (value: string) => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
};

/**
 * Approval of a memory card (V0.6.8). An approval belongs to one revision: editing the card makes a new revision, which
 * is not approved until a reviewer reads it and approves it. With independent review on, the person who made a revision
 * cannot approve it. Every decision is kept with who made it.
 */
export function MemoryApprovalPanel({ accessToken, cardId, isAdmin, updatedAt, workspaceId }: { accessToken: string; cardId: string; isAdmin: boolean; updatedAt: string; workspaceId: string }) {
  const [status, setStatus] = useState<MemoryApprovalStatus | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getSharedMemoryApproval(accessToken, cardId).then((next) => { if (!cancelled) setStatus(next); }).catch((error) => { if (!cancelled) showToast("error", apiMessage(error, "The approval could not be read.")); });
    return () => { cancelled = true; };
  }, [accessToken, cardId, updatedAt]);

  async function run(action: () => Promise<MemoryApprovalStatus>, done: string) {
    setIsSaving(true);
    try {
      setStatus(await action());
      showToast("success", done);
    } catch (error) {
      showToast("error", apiMessage(error, "That could not be saved."));
    } finally {
      setIsSaving(false);
    }
  }

  async function togglePolicy(independent: boolean) {
    try {
      await setSharedApprovalPolicy(accessToken, workspaceId, independent);
      setStatus(await getSharedMemoryApproval(accessToken, cardId));
      showToast("success", independent ? "Independent review is required." : "Independent review is no longer required.");
    } catch (error) {
      showToast("error", apiMessage(error, "The policy could not be changed."));
    }
  }

  if (!status) return null;
  return <section aria-labelledby={`approval-${cardId}`} className="shared-record-panel">
    <PanelHeading description={status.approved ? "Approved" : "Not approved"} icon={Approved} id={`approval-${cardId}`} title="Approval" />
    <p className="shared-muted-copy">
      {status.approved ? `Revision ${status.revision} was approved${status.approved_by ? ` by ${status.approved_by.display_name}` : ""}${status.approved_at ? ` on ${when(status.approved_at)}` : ""}. Any edit makes a new revision that needs a new approval.` : `Revision ${status.revision} has not been approved.`}
      {status.independent_review ? " This workspace asks for independent review: the person who made a revision cannot approve it." : ""}
    </p>
    {status.cannot_approve_reason && !status.approved ? <p className="shared-muted-copy">{status.cannot_approve_reason}</p> : null}
    <div className="shared-detail-actions">
      {status.can_approve ? <Button busy={isSaving} onClick={() => void run(() => approveSharedMemoryCard(accessToken, cardId, status.revision), "Approved.")} type="button" variant="main">Approve revision {status.revision}</Button> : null}
      {status.approved ? <Button disabled={isSaving} onClick={() => void run(() => withdrawSharedMemoryApproval(accessToken, cardId), "Approval withdrawn.")} type="button" variant="secondary">Withdraw approval</Button> : null}
    </div>
    {isAdmin ? <label className="rm-approval-policy"><Switch checked={status.independent_review} onCheckedChange={(checked) => void togglePolicy(checked)} /> Require independent review in this workspace</label> : null}
    {status.history.length ? <ul className="shared-version-list" aria-label="Approval decisions">
      {status.history.map((decision, index) => <li key={index}><div><strong>{decision.decision === "approved" ? "Approved" : "Withdrawn"} · revision {decision.revision}</strong><small>{decision.user ? decision.user.display_name : "Unknown"} · {when(decision.created_at)}{decision.note ? ` · ${decision.note}` : ""}</small></div></li>)}
    </ul> : null}
  </section>;
}

/** Reads the workspace's approval policy; used by screens that only need to know whether independent review is on. */
export async function independentReviewRequired(accessToken: string, workspaceId: string) {
  return (await getSharedApprovalPolicy(accessToken, workspaceId)).independent_review;
}
