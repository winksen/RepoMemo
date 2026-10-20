import { useState } from "react";
import type { FormEvent } from "react";
import { IconArrowRight as ArrowRight, IconLock as Lock, IconShieldLock as ShieldLock, IconUserShield as UserShield } from "@tabler/icons-react";
import { verifySetupCode } from "../../lib/sharedApi";
import { Button } from "../../components/ui/button";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";
import { StepHeading } from "./StepFrame";

export function WelcomeStep({ code, onCode, onVerified }: { code: string; onCode: (code: string) => void; onVerified: () => void }) {
  const [isBusy, setIsBusy] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true);
    try { await verifySetupCode(code); onVerified(); }
    catch (error) { showToast("error", apiMessage(error, "The setup code could not be checked.")); }
    finally { setIsBusy(false); }
  }
  return <>
    <StepHeading eyebrow="Step 1 of 7" title="Set up this RepoMemo server">
      This server has no account yet. You will create its system administrator, check the server, choose the first settings and, if you like, a first workspace.
    </StepHeading>
    <ul className="rm-setup-points">
      <li><ShieldLock size={ICON_SIZE.md} /> Only someone with access to the server can do this: enter the one-time setup code printed on the server console when it started (or the value of <code>REPOMEMO_SETUP_CODE</code>).</li>
      <li><UserShield size={ICON_SIZE.md} /> The account you create administers the whole server: every organization and workspace, its users and its settings.</li>
      <li><Lock size={ICON_SIZE.md} /> Until setup is finished, nobody else can create an account here.</li>
    </ul>
    <form className="shared-setup-form" onSubmit={submit}>
      <Field label="Setup code" required><Input autoComplete="one-time-code" autoFocus onChange={(event) => onCode(event.target.value)} placeholder="ABCD-EFGH-JKLM" spellCheck={false} value={code} /></Field>
      <div className="rm-setup-actions"><span /><Button busy={isBusy} disabled={!code.trim()} type="submit" variant="main"><ArrowRight size={ICON_SIZE.md} /> Continue</Button></div>
    </form>
  </>;
}
