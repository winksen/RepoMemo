import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { IconKey as Key, IconLoader2 as Loader } from "@tabler/icons-react";
import { completeSharedPasswordReset, previewSharedPasswordReset } from "../lib/sharedApi";
import type { PasswordResetPreview } from "../types";
import { ThemeToggle } from "./SharedLayout";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

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
    previewSharedPasswordReset(token).then(setPreview).catch((error) => setProblem(errorMessage(error, "This link could not be checked.")));
  }, [token]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (password !== confirmation) { showToast("error", "The two passwords are not the same."); return; }
    setIsBusy(true);
    try {
      const response = await completeSharedPasswordReset(token, password);
      showToast("success", "Your password was changed. Every other device was signed out.");
      onAuthenticated(response.access_token);
    } catch (error) { showToast("error", errorMessage(error, "The password could not be changed.")); setIsBusy(false); }
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
        </div> : !preview ? <p className="shared-muted-copy"><Loader className="spin" size={ICON_SIZE.md} /> Checking the link…</p> : <>
          <div className="shared-auth-heading">
            <p className="shared-eyebrow">{preview.display_name}</p>
            <h2>New password for {preview.email ?? preview.display_name}</h2>
            <p>Every device signed in to this account is signed out when you save. Personal access tokens keep working; revoke them in your profile if needed.</p>
          </div>
          <form className="shared-auth-form" onSubmit={submit}>
            <input autoComplete="username" hidden readOnly value={preview.email ?? ""} />
            <label>New password<Input autoComplete="new-password" autoFocus maxLength={1024} minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={password} /></label>
            <label>Repeat it<Input autoComplete="new-password" maxLength={1024} minLength={12} onChange={(event) => setConfirmation(event.target.value)} required type="password" value={confirmation} /></label>
            <Button disabled={isBusy} type="submit" variant="main">{isBusy ? <Loader className="spin" size={ICON_SIZE.md} /> : <Key size={ICON_SIZE.md} />} Save the new password</Button>
          </form>
        </>}
      </div>
    </section>
  </main>;
}
