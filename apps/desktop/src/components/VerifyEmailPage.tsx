import { useEffect, useState } from "react";
import { IconCircleCheck as CircleCheck, IconLoader2 as Loader } from "@tabler/icons-react";
import { verifyEmailLink } from "../lib/sharedApi";
import { ThemeToggle } from "./SharedLayout";
import { Button } from "./ui/button";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

/**
 * `/verify-email#<token>`: uses the link mailed to an address. The secret is
 * in the fragment, so it never reaches a server or its logs. It works signed
 * in or not; nothing is signed in by it.
 */
export function VerifyEmailPage({ onContinue, token }: { onContinue: () => void; token: string }) {
  const [email, setEmail] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  useEffect(() => {
    verifyEmailLink(token)
      .then((answer) => setEmail(answer.email))
      .catch((error) => setProblem(apiMessage(error, "This link could not be checked.")));
  }, [token]);

  return <main className="shared-auth-canvas">
    <ThemeToggle className="shared-auth-theme-toggle" />
    <section className="shared-auth-intro" aria-label="Email verification">
      <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
      <p className="shared-eyebrow">Account</p>
      <h1>Confirm your email address</h1>
    </section>
    <section className="shared-auth-form-region" aria-label="Verification result" aria-live="polite">
      <div className="shared-auth-form-shell">
        {problem ? <div className="shared-auth-heading">
          <p className="shared-eyebrow">Email verification</p>
          <h2>This link cannot be used</h2>
          <p>{problem}</p>
          <Button onClick={onContinue} type="button" variant="secondary">Continue</Button>
        </div> : email === null ? <p className="shared-muted-copy"><Loader className="spin" size={ICON_SIZE.md} /> Checking the link…</p> : <div className="shared-auth-heading">
          <p className="shared-eyebrow"><CircleCheck size={ICON_SIZE.sm} /> Confirmed</p>
          <h2>{email} is confirmed</h2>
          <p>You can create organizations and workspaces now.</p>
          <Button onClick={onContinue} type="button" variant="main">Continue to RepoMemo</Button>
        </div>}
      </div>
    </section>
  </main>;
}
