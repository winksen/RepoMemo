import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { IconKey as Key } from "@tabler/icons-react";
import { completeSharedPasswordReset, previewSharedPasswordReset } from "../lib/sharedApi";
import type { PasswordResetPreview } from "../types";
import { ThemeToggle } from "./SharedLayout";
import { Button } from "./ui/button";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { LoadingState } from "./ui/loading-state";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

/**
 * `/reset-password#<token>`: a one-time link from a system administrator.
 * The secret is in the fragment, so it never reaches a server or its logs.
 * Choosing a new password signs out every other device of the account and
 * signs this browser in.
 */
export function PasswordResetPage({ onAuthenticated, token }: { onAuthenticated: (accessToken: string) => void; token: string }) {
  const [preview, setPreview] = useState<PasswordResetPreview | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    previewSharedPasswordReset(token).then(setPreview).catch((error) => setProblem(apiMessage(error, "This link could not be checked.")));
  }, [token]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (password !== confirmation) { showToast("error", "The two passwords are not the same."); return; }
    setIsBusy(true);
    try {
      const response = await completeSharedPasswordReset(token, password);
      showToast("success", "Your password was changed. Every other device was signed out.");
      onAuthenticated(response.access_token);
    } catch (error) { showToast("error", apiMessage(error, "The password could not be changed.")); setIsBusy(false); }
  }

  return <main className="shared-auth-canvas">
    <ThemeToggle className="shared-auth-theme-toggle" />
    <section className="shared-auth-intro" aria-label="Password reset">
      <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
      <p className="shared-eyebrow">Account</p>
      <h1>Choose a new password</h1>
      <p className="shared-intro-copy">This link was created by a system administrator of this server. It works once, and only for the account shown.</p>
    </section>
    <section className="shared-auth-form-region" aria-label="New password" aria-live="polite">
      <div className="shared-auth-form-shell">
        {problem ? <div className="shared-auth-heading">
          <p className="shared-eyebrow">Password reset</p>
          <h2>This link cannot be used</h2>
          <p>{problem}</p>
          <Button onClick={() => window.location.assign("/login")} type="button" variant="secondary">Go to sign-in</Button>
        </div> : !preview ? <LoadingState label="Checking the link" /> : <>
          <div className="shared-auth-heading">
            <p className="shared-eyebrow">{preview.display_name}</p>
            <h2>New password for {preview.email ?? preview.display_name}</h2>
            <p>Every device signed in to this account is signed out when you save. Personal access tokens keep working; revoke them in your profile if needed.</p>
          </div>
          <form className="shared-auth-form" onSubmit={submit}>
            <input autoComplete="username" hidden readOnly value={preview.email ?? ""} />
            <Field label="New password" required><Input autoComplete="new-password" autoFocus maxLength={1024} minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" type="password" value={password} /></Field>
            <Field label="Repeat it" required><Input autoComplete="new-password" maxLength={1024} minLength={12} onChange={(event) => setConfirmation(event.target.value)} type="password" value={confirmation} /></Field>
            <Button busy={isBusy} type="submit" variant="main"><Key size={ICON_SIZE.md} /> Save the new password</Button>
          </form>
        </>}
      </div>
    </section>
  </main>;
}
