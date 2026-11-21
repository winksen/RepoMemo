import { useState } from "react";
import { Question } from "@phosphor-icons/react";
import { submitSharedSearchFeedback } from "../lib/sharedApi";
import { Button } from "./ui/button";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

/**
 * "Not what you needed? Tell the owners" (V0.6.25). It sends what was searched for, and optionally the result that did not
 * help, to the people who can improve the evidence. The search on the page is not touched, so the query and the results stay
 * where they are. The query text is kept for a stated time only: 180 days while open, 30 days after it is answered.
 */
export function SearchFeedbackButton({ accessToken, artifactId, query, workspaceId }: { accessToken: string; artifactId?: string; query: string; workspaceId: string }) {
  const [isOpen, setIsOpen] = useState(false);
  const [note, setNote] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [sent, setSent] = useState(false);
  const kind = artifactId ? "unhelpful_result" : "no_answer";

  async function send() {
    setIsSending(true);
    try {
      await submitSharedSearchFeedback(accessToken, workspaceId, { kind, query, artifactId, note });
      setSent(true);
      setIsOpen(false);
      setNote("");
      showToast("success", "Sent to the people who can improve the evidence. Your search is still here.");
    } catch (error) {
      showToast("error", apiMessage(error, "That could not be sent."));
    } finally { setIsSending(false); }
  }

  if (sent) return <p className="shared-muted-copy" role="status">Sent. You will see the answer under Search requests in your workspace.</p>;
  if (!isOpen) return <Button onClick={() => setIsOpen(true)} type="button" variant="secondary"><Question size={ICON_SIZE.md} /> {artifactId ? "This result did not help" : "Not what you needed? Tell the owners"}</Button>;
  return (
    <form className="rm-search-feedback-form" onSubmit={(event) => { event.preventDefault(); void send(); }}>
      <p className="shared-muted-copy">The owners will see “{query}”{artifactId ? " and the result that did not help" : ""}. It is removed 30 days after it is answered, or after 180 days if nobody answers.</p>
      <Field label="What were you hoping to find? (optional)"><Input maxLength={500} onChange={(event) => setNote(event.target.value)} value={note} /></Field>
      <div className="shared-detail-actions">
        <Button busy={isSending} type="submit" variant="main">Send</Button>
        <Button disabled={isSending} onClick={() => setIsOpen(false)} type="button" variant="secondary">Cancel</Button>
      </div>
    </form>
  );
}
