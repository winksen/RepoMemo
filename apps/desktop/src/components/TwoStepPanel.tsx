// Profile › Two-step sign-in: set up an authenticator app with a first code, keep the recovery codes (shown once),
// replace them or turn it off. Changing it always asks for the password and a code.
import { useCallback, useEffect, useState } from "react";
import { IconShieldLock as Shield } from "@tabler/icons-react";
import { request } from "../lib/sharedApi";
import { apiMessage } from "../web/format";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { CopyButton } from "./ui/copy-button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";

interface Status { enabled: boolean; recovery_codes_remaining: number }
interface Setup { secret: string; otpauth_url: string }

export function TwoStepPanel({ accessToken }: { accessToken: string }) {
  const [status, setStatus] = useState<Status | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [code, setCode] = useState("");
  const [codes, setCodes] = useState<string[] | null>(null);
  const [saved, setSaved] = useState(false);
  const [change, setChange] = useState<"disable" | "regenerate" | null>(null);
  const [password, setPassword] = useState("");
  const [proof, setProof] = useState("");
  const [busy, setBusy] = useState(false);
  const load = useCallback(async () => {
    try { setStatus(await request<Status>("/v1/profile/mfa", {}, accessToken)); }
    catch (error) { showToast("error", apiMessage(error, "Two-step sign-in could not be loaded.")); }
  }, [accessToken]);
  useEffect(() => { void load(); }, [load]);
  if (!status) return null;

  async function begin() {
    setBusy(true);
    try { setSetup(await request<Setup>("/v1/profile/mfa/start", { method: "POST", body: "{}" }, accessToken)); setCode(""); }
    catch (error) { showToast("error", apiMessage(error, "Setup could not start.")); }
    finally { setBusy(false); }
  }
  async function confirm() {
    setBusy(true);
    try {
      const result = await request<{ recovery_codes: string[] }>("/v1/profile/mfa/confirm", { method: "POST", body: JSON.stringify({ code }) }, accessToken);
      setSetup(null); setCode(""); setSaved(false); setCodes(result.recovery_codes);
      await load();
    } catch (error) { showToast("error", apiMessage(error, "That code is not right.")); }
    finally { setBusy(false); }
  }
  async function apply() {
    if (!change) return;
    setBusy(true);
    try {
      const body = JSON.stringify({ password, code: proof });
      if (change === "disable") {
        await request("/v1/profile/mfa/disable", { method: "POST", body }, accessToken);
        showToast("success", "Two-step sign-in is off. Your password alone signs you in again.");
      } else {
        const result = await request<{ recovery_codes: string[] }>("/v1/profile/mfa/recovery-codes", { method: "POST", body }, accessToken);
        setSaved(false); setCodes(result.recovery_codes);
      }
      setChange(null); setPassword(""); setProof("");
      await load();
    } catch (error) { showToast("error", apiMessage(error, "The change could not be made. Check the password and the code.")); }
    finally { setBusy(false); }
  }

  return <section aria-label="Two-step sign-in" className="shared-profile-panel">
    <PanelHeading description={<Badge>{status.enabled ? "on" : "off"}</Badge>} icon={Shield} title="Two-step sign-in" />
    <p className="shared-muted-copy">Adds a six-digit code from an authenticator app to your password. Without a code, even the right password does not sign you in, and neither does a password reset link. It does not apply when you sign in through your organization's provider.</p>
    {codes ? <div role="status">
      <strong>Your recovery codes</strong>
      <p>Each works once if you lose your phone. They are shown only now: keep them somewhere safe, not in this browser.</p>
      <pre aria-label="Recovery codes" role="group">{codes.join("\n")}</pre>
      <CopyButton label="Copy recovery codes" text="Copy the codes" value={codes.join("\n")} />{" "}
      <Button disabled={saved} onClick={() => setSaved(true)} type="button" variant="secondary">I have saved them</Button>
      {saved ? <>{" "}<Button onClick={() => setCodes(null)} type="button" variant="main">Done</Button></> : null}
    </div> : null}
    {!status.enabled && !setup ? <Button busy={busy} onClick={() => void begin()} type="button" variant="secondary">Set up two-step sign-in</Button> : null}
    {setup ? <form onSubmit={(event) => { event.preventDefault(); void confirm(); }}>
      <p>In your authenticator app add an account <a href={setup.otpauth_url}>from this link</a>, or type this key: <code>{setup.secret}</code> <CopyButton label="Copy the key" value={setup.secret} /></p>
      <Field label="First code from the app" hint="It must be right before two-step sign-in is turned on." required><Input autoComplete="one-time-code" inputMode="numeric" maxLength={6} onChange={(event) => setCode(event.target.value)} value={code} /></Field>
      <Button busy={busy} disabled={code.trim().length !== 6} type="submit" variant="main">Turn on</Button>{" "}
      <Button onClick={() => { setSetup(null); setCode(""); }} type="button" variant="secondary">Cancel</Button>
    </form> : null}
    {status.enabled ? <>
      <p>{status.recovery_codes_remaining} recovery code{status.recovery_codes_remaining === 1 ? "" : "s"} left.{status.recovery_codes_remaining <= 2 ? " Replace them soon." : ""}</p>
      <Button onClick={() => setChange("regenerate")} type="button" variant="secondary">Replace recovery codes</Button>{" "}
      <Button onClick={() => setChange("disable")} type="button" variant="secondary">Turn off</Button>
    </> : null}
    <Dialog
      description={change === "disable" ? "Your password alone will sign you in again." : "The old codes stop working at once."}
      footer={<><DialogCancel onClick={() => setChange(null)} /><Button busy={busy} disabled={!password || !proof.trim()} type="submit" variant={change === "disable" ? "danger" : "main"}>{change === "disable" ? "Turn off" : "Replace codes"}</Button></>}
      onClose={() => { setChange(null); setPassword(""); setProof(""); }}
      onSubmit={() => void apply()}
      open={change !== null}
      title={change === "disable" ? "Turn off two-step sign-in?" : "Replace your recovery codes?"}
    >
      <Field label="Password" required><Input autoComplete="current-password" onChange={(event) => setPassword(event.target.value)} type="password" value={password} /></Field>
      <Field label="Code" hint="From your authenticator app, or an unused recovery code." required><Input autoComplete="one-time-code" onChange={(event) => setProof(event.target.value)} value={proof} /></Field>
    </Dialog>
  </section>;
}
