import { type FormEvent, useEffect, useState } from "react";
import { IconArrowRight as ArrowRight, IconKey as Key, IconShieldLock as Shield, IconUsers as Users } from "@tabler/icons-react";
import { DesktopServerNote } from "../components/DesktopServerNote";
import { SharedApiError, fetchSignInProvider, loginSharedUser, requestPasswordResetMail, sharedApiUrl, signUpSharedUser, startProviderFlow } from "../lib/sharedApi";
import { Button } from "../components/ui/button";
import { Field } from "../components/ui/field";
import { SegmentedControl } from "../components/ui/segmented-control";
import { Input } from "../components/ui/input";
import { Toast, showToast } from "../components/ui/toast";
import { ProviderSignIn } from "../components/ProviderSignIn";
import { AuthFrame } from "../components/AuthFrame";
import { type AuthMode } from "./constants";
import { apiMessage } from "./format";
import { navigate } from "./router";
import { ICON_SIZE } from "../components/ui/icon";

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
  const [forgotOpen, setForgotOpen] = useState(false);
  const [needsCode, setNeedsCode] = useState(false);
  const [mfaCode, setMfaCode] = useState("");
  const [provider, setProvider] = useState<string | null>(null);
  const [enforced, setEnforced] = useState(false);
  const [breakGlass, setBreakGlass] = useState(false);

  useEffect(() => { void fetchSignInProvider().then((answer) => { setProvider(answer.name); setEnforced(answer.enforced); }).catch(() => setProvider(null)); }, []);
  // Enforced provider sign-in: the password form is only a break-glass door for administrators.
  const passwordHidden = enforced && !breakGlass && !setupPending;

  useEffect(() => {
    setMode(setupPending ? "sign-in" : initialMode);
    setError(initialError);
  }, [initialError, initialMode, setupPending]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true);
    setError(null);
    try {
      if (mode === "sign-up") {
        const answer = await signUpSharedUser({ email, displayName, password });
        if ("session" in answer) {
          onAuthenticated(answer.session.access_token);
        } else {
          // The server answers the same for every address: sign in next.
          showToast("success", answer.message);
          setPassword("");
          navigate("/login");
          setMode("sign-in");
        }
        return;
      }
      const response = await loginSharedUser({ email, password, ...(needsCode && mfaCode.trim() ? { mfa_code: mfaCode.trim() } : {}) });
      onAuthenticated(response.access_token);
    } catch (requestError) {
      // The password was right; the account also needs its second factor.
      if (requestError instanceof SharedApiError && requestError.code === "mfa_required") { setNeedsCode(true); setMfaCode(""); }
      setError(apiMessage(requestError));
    } finally {
      setIsSubmitting(false);
    }
  }

  async function forgot() {
    if (!email.trim()) { setError("Enter your email address first."); return; }
    setIsSubmitting(true);
    setError(null);
    try {
      const answer = await requestPasswordResetMail(email.trim());
      showToast("success", answer.message);
      setForgotOpen(false);
    } catch (requestError) {
      setError(apiMessage(requestError));
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <AuthFrame
      copy="Sign in to the server-authoritative workspace. Your browser only receives data allowed by your signed session."
      eyebrow="Shared workspace"
      formLabel="Account access"
      introLabel="RepoMemo shared mode"
      title="One evidence base for the team."
      trust={[
        { icon: <Shield size={ICON_SIZE.md} />, title: "Server boundary", text: "Shared data is requested through the protected API." },
        { icon: <Key size={ICON_SIZE.md} />, title: "Session token", text: "Access expires and is kept only for this browser session." },
        { icon: <Users size={ICON_SIZE.md} />, title: "Membership-aware", text: "Workspace access is scoped to the signed-in account." },
      ]}
    >
      {setupPending || enforced ? null : <SegmentedControl aria-label="Account action" fill onValueChange={(next) => { navigate(next === "sign-in" ? "/login" : "/register"); setMode(next); setError(null); }} options={[{ label: "Sign in", value: "sign-in" }, { label: "Create account", value: "sign-up" }]} value={mode} />}
      {setupPending ? <div className="shared-auth-heading">
        <p className="shared-eyebrow">Server setup</p>
        <h2>Finish setting up RepoMemo</h2>
        <p>This server's setup is not finished. Sign in as the system administrator created during setup to complete it. Nobody else can sign in or register until then.</p>
      </div> : <div className="shared-auth-heading">
        <p className="shared-eyebrow">{mode === "sign-in" ? "Welcome back" : "Start a shared workspace"}</p>
        <h2>{mode === "sign-in" ? "Continue to RepoMemo" : "Create your account"}</h2>
        <p>{mode === "sign-in" ? "Use the credentials registered with this server." : "Your account will be the initial owner of any organization you create."}</p>
      </div>}
      {mode === "sign-in" && provider ? <ProviderSignIn label={`Continue with ${provider}`} onDone={(session) => onAuthenticated(session.access_token)} start={() => startProviderFlow()} /> : null}
      {passwordHidden ? <p className="shared-auth-hint">This server requires {provider}. <Button onClick={() => setBreakGlass(true)} type="button" variant="secondary">Administrator break-glass sign-in</Button></p> : <form className="shared-auth-form" onSubmit={submit}>
        {mode === "sign-up" ? <Field label="Display name" required><Input autoComplete="name" onChange={(event) => setDisplayName(event.target.value)} placeholder="Ada Lovelace" value={displayName} /></Field> : null}
        <Field label="Email" required><Input autoComplete="email" onChange={(event) => setEmail(event.target.value)} placeholder="you@company.com" type="email" value={email} /></Field>
        <Field label="Password" required><Input autoComplete={mode === "sign-up" ? "new-password" : "current-password"} minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" type="password" value={password} /></Field>
        {mode === "sign-in" && needsCode ? <Field label="Authentication code" hint="From your authenticator app, or one of your recovery codes." required><Input autoComplete="one-time-code" autoFocus onChange={(event) => setMfaCode(event.target.value)} value={mfaCode} /></Field> : null}
        <Toast kind="error" message={error} />
        <Button busy={isSubmitting} type="submit" variant="main">
          <ArrowRight size={ICON_SIZE.md} />
          {mode === "sign-in" ? "Sign in to shared mode" : "Create account and continue"}
        </Button>
      </form>}
      {mode === "sign-in" && !passwordHidden ? <div className="shared-auth-hint">
        {forgotOpen
          ? <p>We will send a link to <strong>{email.trim() || "the address above"}</strong> if an account uses it and this server sends mail. It works once, for one hour. <Button busy={isSubmitting} onClick={() => void forgot()} variant="secondary">Send the link</Button> <Button onClick={() => setForgotOpen(false)} variant="secondary">Cancel</Button></p>
          : <p>Forgot your password? <Button onClick={() => setForgotOpen(true)} variant="secondary">Get a link by email</Button>, or ask a system administrator of this server for a reset link (System › Users).</p>}
      </div> : <p className="shared-auth-hint">Invited by a teammate? Open the invitation link you received instead: it creates the account with the right access.</p>}
      <p className="shared-api-note">API {apiAvailable === true ? "healthy" : apiAvailable === false ? "unreachable" : "checking"} · <code>{sharedApiUrl || "this address"}</code></p>
      <DesktopServerNote signedIn={false} />
    </AuthFrame>
  );
}
