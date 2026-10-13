import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { IconArrowRight as ArrowRight, IconLoader2 as Loader, IconMailForward as MailForward } from "@tabler/icons-react";
import { acceptSharedInvitation, loginSharedUser, previewSharedInvitation, registerSharedUser } from "../lib/sharedApi";
import type { AcceptedInvitation, InvitationPreview, SharedSession } from "../types";
import { ThemeToggle } from "./SharedLayout";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** Where to go once the invitation is accepted. */
export function invitationDestination(accepted: AcceptedInvitation): string {
  return accepted.target_type === "workspace"
    ? `/workspaces/${encodeURIComponent(accepted.target_id)}/overview`
    : `/workspaces?organization=${encodeURIComponent(accepted.target_id)}`;
}

/**
 * `/invite/<token>`: what the invitation is for, then the shortest way in.
 * Signed in with the invited address: join. Not signed in: sign in, or create
 * the account (allowed by the invitation even when registration is closed),
 * then join. Signed in as someone else: say so.
 */
export function InvitePage({
  accessToken,
  onAuthenticated,
  onJoined,
  onSignOut,
  session,
  token,
}: {
  accessToken: string | null;
  /** A new session was created here; continue to `destination`. */
  onAuthenticated: (accessToken: string, destination: string) => void;
  /** The signed-in user joined; reload their workspaces and continue. */
  onJoined: (destination: string) => void;
  onSignOut: () => void;
  session: SharedSession | null;
  token: string;
}) {
  const [preview, setPreview] = useState<InvitationPreview | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    previewSharedInvitation(token).then(setPreview).catch((error) => setProblem(errorMessage(error, "This invitation link could not be checked.")));
  }, [token]);

  const signedInEmail = session?.user.email?.toLowerCase() ?? null;
  const isInvitedAccount = Boolean(preview && signedInEmail === preview.email);
  const place = preview ? (preview.target_type === "workspace" ? `the ${preview.target_name} workspace` : `the ${preview.target_name} organization`) : "";

  async function joinAs(token_: string) {
    return invitationDestination(await acceptSharedInvitation(token_, token));
  }

  async function joinSignedIn() {
    if (!accessToken) return;
    setIsBusy(true);
    try {
      const destination = await joinAs(accessToken);
      showToast("success", `You joined ${place}.`);
      onJoined(destination);
    } catch (error) { showToast("error", errorMessage(error, "The invitation could not be accepted.")); setIsBusy(false); }
  }

  async function signInAndJoin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!preview) return;
    setIsBusy(true);
    try {
      const response = preview.account_exists
        ? await loginSharedUser({ email: preview.email, password })
        : await registerSharedUser({ email: preview.email, displayName, password, invitationToken: token });
      // Signed in from here on; joining can be retried from this page if it fails.
      try {
        const destination = await joinAs(response.access_token);
        showToast("success", `You joined ${place}.`);
        onAuthenticated(response.access_token, destination);
      } catch (error) {
        showToast("error", errorMessage(error, "You are signed in, but the invitation could not be accepted."));
        onAuthenticated(response.access_token, `/invite/${token}`);
      }
    } catch (error) { showToast("error", errorMessage(error, preview.account_exists ? "Sign-in failed." : "The account could not be created.")); setIsBusy(false); }
  }

  return <main className="shared-auth-canvas">
    <ThemeToggle className="shared-auth-theme-toggle" />
    <section className="shared-auth-intro" aria-label="Invitation">
      <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
      <p className="shared-eyebrow">Invitation</p>
      <h1>{preview ? `Join ${preview.target_name}` : "You have been invited"}</h1>
      {preview ? <dl className="shared-trust-list">
        <div><dt>{preview.target_type === "workspace" ? "Workspace" : "Organization"}</dt><dd>{preview.target_name}{preview.target_type === "workspace" ? ` · in ${preview.organization_name}` : ""}</dd></div>
        <div><dt>Role</dt><dd>{preview.role}</dd></div>
        <div><dt>For</dt><dd>{preview.email}</dd></div>
        <div><dt>From</dt><dd>{preview.invited_by ?? "An administrator"} · valid until {formatDate(preview.expires_at)}</dd></div>
      </dl> : null}
    </section>
    <section className="shared-auth-form-region" aria-label="Accept the invitation" aria-live="polite">
      <div className="shared-auth-form-shell">
        {problem ? <div className="shared-auth-heading">
          <p className="shared-eyebrow">Invitation</p>
          <h2>This link cannot be used</h2>
          <p>{problem}</p>
          <Button onClick={() => window.location.assign(session ? "/dashboard" : "/login")} type="button" variant="secondary">{session ? "Go to the dashboard" : "Go to sign-in"}</Button>
        </div> : !preview ? <p className="shared-muted-copy"><Loader className="spin" size={16} /> Checking the invitation…</p> : session ? (isInvitedAccount ? <div className="shared-auth-heading">
          <p className="shared-eyebrow">Signed in as {session.user.display_name}</p>
          <h2>Join {place}</h2>
          <p>You will join as {preview.role}. If you already have a higher role there, you keep it.</p>
          <Button disabled={isBusy} onClick={() => void joinSignedIn()} type="button" variant="main">{isBusy ? <Loader className="spin" size={17} /> : <MailForward size={17} />} Accept the invitation</Button>
        </div> : <div className="shared-auth-heading">
          <p className="shared-eyebrow">Another account</p>
          <h2>This invitation is for {preview.email}</h2>
          <p>You are signed in as {session.user.email ?? session.user.display_name}. Sign out, then open the link again to accept it with the invited account.</p>
          <Button onClick={onSignOut} type="button" variant="secondary">Sign out</Button>
        </div>) : <>
          <div className="shared-auth-heading">
            <p className="shared-eyebrow">{preview.account_exists ? "Welcome back" : "New account"}</p>
            <h2>{preview.account_exists ? "Sign in to accept" : "Create your account to accept"}</h2>
            <p>{preview.account_exists ? `An account exists for ${preview.email}. Sign in and you join ${place} right away.` : `Your account uses ${preview.email}. You join ${place} as soon as it is created.`}</p>
          </div>
          <form className="shared-auth-form" onSubmit={signInAndJoin}>
            {preview.account_exists ? null : <label>Display name<Input autoComplete="name" maxLength={120} onChange={(event) => setDisplayName(event.target.value)} placeholder="Ada Lovelace" required value={displayName} /></label>}
            <label>Email<Input autoComplete="email" readOnly type="email" value={preview.email} /></label>
            <label>Password<Input autoComplete={preview.account_exists ? "current-password" : "new-password"} minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={password} /></label>
            <Button disabled={isBusy} type="submit" variant="main">{isBusy ? <Loader className="spin" size={17} /> : <ArrowRight size={17} />} {preview.account_exists ? "Sign in and join" : "Create account and join"}</Button>
          </form>
        </>}
      </div>
    </section>
  </main>;
}
