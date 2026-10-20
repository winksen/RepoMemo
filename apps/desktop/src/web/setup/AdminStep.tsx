import { useState } from "react";
import type { FormEvent } from "react";
import { IconArrowLeft as ArrowLeft, IconUserShield as UserShield } from "@tabler/icons-react";
import { createSetupAdmin, SharedApiError } from "../../lib/sharedApi";
import { Button } from "../../components/ui/button";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";
import { StepHeading } from "./StepFrame";

export function AdminStep({ code, onBack, onCodeRejected, onCreated }: { code: string; onBack: () => void; onCodeRejected: () => void; onCreated: (accessToken: string) => void }) {
  const [displayName, setDisplayName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (password !== confirmation) { showToast("error", "The two passwords do not match."); return; }
    setIsBusy(true);
    try {
      const response = await createSetupAdmin({ code, email, displayName, password });
      showToast("success", "System administrator created. You are signed in.");
      onCreated(response.access_token);
    } catch (error) {
      showToast("error", apiMessage(error, "The administrator could not be created."));
      // The code changes when the server restarts; ask for it again.
      if (error instanceof SharedApiError && error.status === 403) onCodeRejected();
    } finally { setIsBusy(false); }
  }
  return <>
    <StepHeading eyebrow="Step 2 of 7" title="Create the system administrator">
      Use an address you keep long-term and a strong password: this account can change everything on the server.
    </StepHeading>
    <form className="shared-setup-form" onSubmit={submit}>
      <Field label="Name" required><Input autoComplete="name" autoFocus maxLength={120} onChange={(event) => setDisplayName(event.target.value)} placeholder="Ada Lovelace" value={displayName} /></Field>
      <Field label="Email" required><Input autoComplete="email" onChange={(event) => setEmail(event.target.value)} placeholder="admin@company.com" type="email" value={email} /></Field>
      <Field label="Password" required><Input autoComplete="new-password" minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" type="password" value={password} /></Field>
      <Field label="Confirm password" required><Input autoComplete="new-password" minLength={12} onChange={(event) => setConfirmation(event.target.value)} type="password" value={confirmation} /></Field>
      <div className="rm-setup-actions"><Button disabled={isBusy} onClick={onBack} variant="secondary"><ArrowLeft size={ICON_SIZE.md} /> Back</Button><Button busy={isBusy} type="submit" variant="main"><UserShield size={ICON_SIZE.md} /> Create administrator</Button></div>
    </form>
  </>;
}
