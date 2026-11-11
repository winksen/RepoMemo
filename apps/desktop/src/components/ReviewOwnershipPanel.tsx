import { useEffect, useState } from "react";
import { IconUserCheck as UserCheck } from "@tabler/icons-react";
import { completeSharedReview, getSharedReviewAssignment, listSharedWorkspaceMembers, setSharedReviewAssignment } from "../lib/sharedApi";
import type { ReviewAssignmentView, WorkspaceMember } from "../types";
import { Button } from "./ui/button";
import { Combobox } from "./ui/combobox";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { apiMessage } from "../web/format";

const day = (value: string | null) => (value ? value.slice(0, 10) : "");

/**
 * Who is accountable for reviewing this file or memory card, and when it is next due (V0.6.6). The reviewer may be
 * someone other than the owner; a reviewer who has left the workspace is named as having left. Marking it reviewed
 * records the time and sets the next date.
 */
export function ReviewOwnershipPanel({ accessToken, canWrite, itemId, kind, workspaceId }: { accessToken: string; canWrite: boolean; itemId: string; kind: "evidence" | "memory_card"; workspaceId: string }) {
  const [assignment, setAssignment] = useState<ReviewAssignmentView | null>(null);
  const [members, setMembers] = useState<WorkspaceMember[]>([]);
  const [reviewerId, setReviewerId] = useState("");
  const [due, setDue] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  function take(next: ReviewAssignmentView) {
    setAssignment(next);
    setReviewerId(next.reviewer && !next.reviewer_departed ? next.reviewer.id : "");
    setDue(day(next.next_review_at));
  }

  useEffect(() => {
    let cancelled = false;
    getSharedReviewAssignment(accessToken, workspaceId, kind, itemId).then((next) => { if (!cancelled) take(next); }).catch((error) => { if (!cancelled) showToast("error", apiMessage(error, "The reviewer could not be read.")); });
    if (canWrite) listSharedWorkspaceMembers(accessToken, workspaceId).then((next) => { if (!cancelled) setMembers(next); }).catch(() => undefined);
    return () => { cancelled = true; };
  }, [accessToken, workspaceId, kind, itemId, canWrite]);

  async function save() {
    setIsSaving(true);
    try {
      take(await setSharedReviewAssignment(accessToken, workspaceId, { kind, id: itemId, reviewer_user_id: reviewerId || null, next_review_at: due || null }));
      showToast("success", "Review ownership saved.");
    } catch (error) {
      showToast("error", apiMessage(error, "That could not be saved."));
    } finally {
      setIsSaving(false);
    }
  }

  async function markReviewed() {
    setIsSaving(true);
    try {
      take(await completeSharedReview(accessToken, workspaceId, { kind, id: itemId, next_review_at: due && !assignment?.overdue ? due : null }));
      showToast("success", "Marked as reviewed. Choose the next review date if there should be one.");
    } catch (error) {
      showToast("error", apiMessage(error, "That could not be recorded."));
    } finally {
      setIsSaving(false);
    }
  }

  if (!assignment) return null;
  const options = members.filter((member) => member.role !== "viewer").map((member) => ({ label: member.user.display_name, description: member.user.email ?? undefined, value: member.user.id }));
  const reviewerName = assignment.reviewer ? assignment.reviewer.display_name : assignment.reviewer_label;
  return <section aria-labelledby={`review-owner-${itemId}`} className="shared-record-panel">
    <PanelHeading description={assignment.overdue ? "Review is overdue" : assignment.next_review_at ? `Next review ${day(assignment.next_review_at)}` : "No review date"} icon={UserCheck} id={`review-owner-${itemId}`} title="Reviewer" />
    <p className="shared-muted-copy">
      {reviewerName ? <>{reviewerName}{assignment.reviewer_departed ? " has left this workspace, so nobody is accountable. Choose someone else." : " is accountable for checking this."}</> : "Nobody is accountable for reviewing this yet."}
      {assignment.last_reviewed_at ? ` Last reviewed ${day(assignment.last_reviewed_at)}.` : ""}
    </p>
    {canWrite ? <div className="rm-review-owner-form">
      <Field label="Reviewer"><Combobox aria-label="Reviewer" noneLabel="Nobody" onValueChange={setReviewerId} options={options} placeholder="Choose a reviewer" value={reviewerId} /></Field>
      <Field label="Next review"><Input onChange={(event) => setDue(event.target.value)} type="date" value={due} /></Field>
      <div className="shared-detail-actions">
        <Button busy={isSaving} onClick={() => void save()} type="button" variant="secondary">Save</Button>
        {assignment.reviewer && !assignment.reviewer_departed ? <Button disabled={isSaving} onClick={() => void markReviewed()} type="button" variant="main">Mark reviewed</Button> : null}
      </div>
    </div> : null}
  </section>;
}
