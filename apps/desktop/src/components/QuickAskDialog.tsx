import { useEffect, useState } from "react";
import { sendToAssistant } from "../lib/assistantBridge";
import { ASSISTANT_NAME } from "../lib/assistantName";
import type { Organization, SharedWorkspace } from "../types";
import { cairnContextWorkspace, rememberCairnWorkspace, rememberedCairnWorkspace } from "../web/cairn";
import { cairnPath, navigate } from "../web/router";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Dropdown } from "./ui/dropdown";
import { Field } from "./ui/field";
import { Textarea } from "./ui/textarea";

/**
 * A quick question for CAIRN, from anywhere: the question and the workspace it is about. Asking opens CAIRN's page on a new chat
 * that sends it at once, so the answer, its sources and the rest of the conversation are there.
 */
export function QuickAskDialog({ onClose, open, organizations, workspaces }: { onClose: () => void; open: boolean; organizations: Organization[]; workspaces: SharedWorkspace[] }) {
  const [question, setQuestion] = useState("");
  const [workspaceId, setWorkspaceId] = useState("");

  useEffect(() => {
    if (!open) return;
    setQuestion("");
    setWorkspaceId(cairnContextWorkspace(workspaces, rememberedCairnWorkspace())?.workspace.id ?? "");
  }, [open]);

  function ask() {
    const message = question.trim();
    if (!message || !workspaceId) return;
    rememberCairnWorkspace(workspaceId);
    navigate(cairnPath(null));
    sendToAssistant({ capability: null, message });
    onClose();
  }

  const label = (entry: SharedWorkspace) => {
    const organization = organizations.find((candidate) => candidate.id === entry.organization_id);
    return organization && organizations.length > 1 ? `${organization.name} / ${entry.workspace.name}` : entry.workspace.name;
  };

  if (!workspaces.length) {
    return <Dialog description={`${ASSISTANT_NAME} works on the evidence of a workspace. Create one, or ask an administrator to add you to one.`} footer={<DialogCancel label="Close" onClick={onClose} />} onClose={onClose} open={open} title="Quick question" />;
  }
  return <Dialog
    description={`Ask one thing. ${ASSISTANT_NAME} answers in a new chat, with its sources.`}
    footer={<><DialogCancel onClick={onClose} /><Button disabled={!question.trim()} type="submit" variant="main">Ask</Button></>}
    onClose={onClose}
    onSubmit={ask}
    open={open}
    size="md"
    title="Quick question"
  >
    <Field label="Your question" required><Textarea autoFocus onChange={(event) => setQuestion(event.target.value)} placeholder="Where is the payment retry implemented?" rows={3} value={question} /></Field>
    <Field label="About this workspace">{(control) => <Dropdown id={control.id} onValueChange={setWorkspaceId} options={workspaces.map((entry) => ({ label: label(entry), value: entry.workspace.id }))} value={workspaceId} />}</Field>
  </Dialog>;
}
