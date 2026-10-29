// Offboarding in one guarded step: shows what the account owns and holds, asks who receives it, then does it and
// reports whether everything ended.
import { useEffect, useRef, useState } from "react";
import { request } from "../lib/sharedApi";
import type { SystemUser } from "../types";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Dropdown } from "./ui/dropdown";
import { Field } from "./ui/field";
import { LoadingState } from "./ui/loading-state";
import { showToast } from "./ui/toast";

interface Preview {
  owned: { kind: string; id: string; name: string }[];
  open_tasks: number;
  tasks: { id: string; workspace: string; title: string; review: boolean }[];
  sessions: number;
  personal_tokens: number;
  invitations: number;
  last_system_administrator: boolean;
}
interface Result { ownership_transferred: number; tasks_reassigned: number; personal_tokens_revoked: number; invitations_withdrawn: number; verified: boolean }

export function OffboardDialog({ accessToken, candidates, onClose, onDone, user }: { accessToken: string; candidates: SystemUser[]; onClose: () => void; onDone: () => void; user: SystemUser }) {
  const [preview, setPreview] = useState<Preview | null>(null);
  const [recipient, setRecipient] = useState("");
  const [busy, setBusy] = useState(false);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    let current = true;
    request<Preview>(`/v1/system/users/${user.id}/offboarding`, {}, accessToken)
      .then((answer) => { if (current) setPreview(answer); })
      .catch((error) => { showToast("error", apiMessage(error, "What this account holds could not be loaded.")); close.current(); });
    return () => { current = false; };
  }, [accessToken, user.id]);
  const hasWork = Boolean(preview && (preview.owned.length > 0 || preview.open_tasks > 0));
  const people = candidates.filter((person) => person.id !== user.id && !person.deactivated_at);
  async function offboard() {
    setBusy(true);
    try {
      const result = await request<Result>(`/v1/system/users/${user.id}/offboard`, { method: "POST", body: JSON.stringify({ transfer_to: recipient || null }) }, accessToken);
      showToast(result.verified ? "success" : "warning", `${user.display_name} is offboarded: ${result.ownership_transferred} owned place${result.ownership_transferred === 1 ? "" : "s"} and ${result.tasks_reassigned} task${result.tasks_reassigned === 1 ? "" : "s"} handed over, ${result.personal_tokens_revoked} token${result.personal_tokens_revoked === 1 ? "" : "s"} revoked. ${result.verified ? "Nothing of their access remains." : "Some access could not be confirmed ended: check the audit trail."}`);
      onDone();
    } catch (error) { showToast("error", apiMessage(error, "The account could not be offboarded. Nothing was changed.")); }
    finally { setBusy(false); }
  }
  return <Dialog
    description="Hands over what they own and their open tasks and reviews, then deactivates the account: sessions and tokens end and the invitations they sent are withdrawn. What they wrote stays attributed to them."
    footer={<><DialogCancel onClick={onClose} /><Button busy={busy} disabled={!preview || preview.last_system_administrator || (hasWork && !recipient)} type="submit" variant="danger">Offboard account</Button></>}
    onClose={onClose}
    onSubmit={() => void offboard()}
    open
    size="lg"
    title={`Offboard ${user.display_name}?`}
  >
    {!preview ? <LoadingState label="Checking what this account holds" /> : <>
      {preview.last_system_administrator ? <p role="alert">This is the last system administrator. Grant the role to someone else first.</p> : null}
      <ul aria-label="What offboarding involves">
        <li>{preview.owned.length ? `Owns ${preview.owned.map((place) => `${place.kind} “${place.name}”`).join(", ")}` : "Owns nothing"}</li>
        <li>{preview.open_tasks ? `${preview.open_tasks} open task${preview.open_tasks === 1 ? "" : "s"} or review${preview.open_tasks === 1 ? "" : "s"} assigned` : "No open tasks or reviews"}{preview.tasks.slice(0, 5).map((task) => <span key={task.id}><br />{task.review ? "Review" : "Task"} · {task.workspace} · {task.title}</span>)}</li>
        <li>{preview.sessions} active session{preview.sessions === 1 ? "" : "s"}, {preview.personal_tokens} personal token{preview.personal_tokens === 1 ? "" : "s"} and {preview.invitations} pending invitation{preview.invitations === 1 ? "" : "s"} sent end now</li>
      </ul>
      {hasWork ? <Field label="Hand everything over to" required hint="Must be able to open each workspace with open tasks and be a member of each place that changes owner. The check happens before anything changes.">{(control) => <Dropdown id={control.id} onValueChange={setRecipient} options={[{ label: "Choose a person", value: "" }, ...people.map((person) => ({ label: `${person.display_name} · ${person.email}`, value: person.id }))]} value={recipient} />}</Field> : null}
    </>}
  </Dialog>;
}
