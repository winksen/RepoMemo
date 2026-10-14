import { type FormEvent, useEffect, useState } from "react";
import { IconArrowRight as ArrowRight, IconKey as Key, IconLoader2 as Loader, IconShieldLock as Shield, IconUsers as Users } from "@tabler/icons-react";
import { loginSharedUser, registerSharedUser, sharedApiUrl } from "../lib/sharedApi";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Toast } from "../components/ui/toast";
import { ThemeToggle } from "../components/SharedLayout";
import { type AuthMode } from "./constants";
import { apiMessage } from "./format";
import { navigate } from "./router";

export function AuthCanvas({
  apiAvailable,
  initialMode,
  initialError,
  onAuthenticated,
  setupPending = false,
}: {
  apiAvailable: boolean | null;
  initialMode: AuthMode;
  initialError: string | null;
  onAuthenticated: (token: string) => void;
  /** The server's setup is not finished: only its system administrator can sign in to complete it. */
  setupPending?: boolean;
}) {
  const [mode, setMode] = useState<AuthMode>(setupPending ? "sign-in" : initialMode);
  const [displayName, setDisplayName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(initialError);

  useEffect(() => {
    setMode(setupPending ? "sign-in" : initialMode);
    setError(initialError);
  }, [initialError, initialMode, setupPending]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true);
    setError(null);
    try {
      const response = mode === "sign-up"
        ? await registerSharedUser({ email, displayName, password })
        : await loginSharedUser({ email, password });
      onAuthenticated(response.access_token);
    } catch (requestError) {
      setError(apiMessage(requestError));
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <main className="shared-auth-canvas">
      <ThemeToggle className="shared-auth-theme-toggle" />
      <section className="shared-auth-intro" aria-label="RepoMemo shared mode">
        <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
        <p className="shared-eyebrow">Shared workspace</p>
        <h1>One evidence base for the team.</h1>
        <p className="shared-intro-copy">Sign in to the server-authoritative workspace. Your browser only receives data allowed by your signed session.</p>
        <dl className="shared-trust-list">
          <div><dt><Shield size={16} /> Server boundary</dt><dd>Shared data is requested through the protected API.</dd></div>
          <div><dt><Key size={16} /> Session token</dt><dd>Access expires and is kept only for this browser session.</dd></div>
          <div><dt><Users size={16} /> Membership-aware</dt><dd>Workspace access is scoped to the signed-in account.</dd></div>
        </dl>
      </section>

      <section className="shared-auth-form-region" aria-label="Account access">
        <div className="shared-auth-form-shell">
          {setupPending ? null : <div className="shared-auth-switch" role="tablist" aria-label="Account action">
            <Button className={mode === "sign-in" ? "active" : ""} onClick={() => { navigate("/login"); setMode("sign-in"); setError(null); }} role="tab" type="button" variant="secondary">Sign in</Button>
            <Button className={mode === "sign-up" ? "active" : ""} onClick={() => { navigate("/register"); setMode("sign-up"); setError(null); }} role="tab" type="button" variant="secondary">Create account</Button>
          </div>}
          {setupPending ? <div className="shared-auth-heading">
            <p className="shared-eyebrow">Server setup</p>
            <h2>Finish setting up RepoMemo</h2>
            <p>This server's setup is not finished. Sign in as the system administrator created during setup to complete it. Nobody else can sign in or register until then.</p>
          </div> : <div className="shared-auth-heading">
            <p className="shared-eyebrow">{mode === "sign-in" ? "Welcome back" : "Start a shared workspace"}</p>
            <h2>{mode === "sign-in" ? "Continue to RepoMemo" : "Create your account"}</h2>
            <p>{mode === "sign-in" ? "Use the credentials registered with this server." : "Your account will be the initial owner of any organization you create."}</p>
          </div>}
          <form className="shared-auth-form" onSubmit={submit}>
            {mode === "sign-up" ? <label>Display name<Input autoComplete="name" onChange={(event) => setDisplayName(event.target.value)} placeholder="Ada Lovelace" required value={displayName} /></label> : null}
            <label>Email<Input autoComplete="email" onChange={(event) => setEmail(event.target.value)} placeholder="you@company.com" required type="email" value={email} /></label>
            <label>Password<Input autoComplete={mode === "sign-up" ? "new-password" : "current-password"} minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={password} /></label>
            <Toast kind="error" message={error} />
            <Button disabled={isSubmitting} type="submit" variant="main">
              {isSubmitting ? <Loader className="spin" size={17} /> : <ArrowRight size={17} />}
              {mode === "sign-in" ? "Sign in to shared mode" : "Create account and continue"}
            </Button>
          </form>
          {mode === "sign-in" ? <p className="shared-auth-hint">Forgot your password? Ask a system administrator of this server for a reset link (System › Users).</p> : <p className="shared-auth-hint">Invited by a teammate? Open the invitation link you received instead: it creates the account with the right access.</p>}
          <p className="shared-api-note">API {apiAvailable === true ? "healthy" : apiAvailable === false ? "unreachable" : "checking"} · <code>{sharedApiUrl}</code></p>
        </div>
      </section>
    </main>
  );
}
