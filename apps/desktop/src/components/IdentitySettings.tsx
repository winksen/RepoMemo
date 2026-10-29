// Extends the System settings form: provider identity, callback, credentials, then
// check/save. Uses the existing neutral fields, loading state and toast feedback.
import { useEffect, useRef, useState } from "react";
import { IconKey } from "@tabler/icons-react";
import { request } from "../lib/sharedApi";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { Switch } from "./ui/switch";
import { LoadingState } from "./ui/loading-state";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { useUnsavedChanges } from "../lib/unsavedChanges";

interface Provider { name: string; issuer: string; client_id: string; enabled: boolean; allowed_domains: string[]; checked_at?: string | null }
interface Overview { provider: Provider | null; has_secret: boolean; callback_url: string | null }
const empty: Provider = { name: "", issuer: "", client_id: "", enabled: false, allowed_domains: [] };

export function IdentitySettings({ accessToken }: { accessToken: string }) {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [draft, setDraft] = useState(empty);
  const [secret, setSecret] = useState("");
  const [domains, setDomains] = useState("");
  const [action, setAction] = useState<"check" | "save" | null>(null);
  const busy = action !== null;
  const form = useRef<HTMLFormElement>(null);
  const [failed, setFailed] = useState(false);
  const [dirty, setDirty] = useState(false);
  useUnsavedChanges(dirty, { subject: "the identity provider settings" });
  function apply(value: Overview) { setOverview(value); setDraft(value.provider ?? empty); setDomains(value.provider?.allowed_domains.join(", ") ?? ""); setSecret(""); setDirty(false); }
  async function load() {
    setFailed(false);
    try { apply(await request<Overview>("/v1/system/identity", {}, accessToken)); }
    catch (error) { setFailed(true); showToast("error", apiMessage(error, "Provider settings could not be loaded. Try again.")); }
  }
  useEffect(() => { void load(); }, [accessToken]);
  function edit(value: Partial<Provider>) { setDraft(current => ({ ...current, ...value })); setDirty(true); }
  async function submit(check: boolean) {
    if (!form.current?.reportValidity()) return;
    setAction(check ? "check" : "save");
    try {
      const result = await request<Overview & { message?: string }>(`/v1/system/identity${check ? "/check" : ""}`, {
        method: check ? "POST" : "PUT", body: JSON.stringify({ ...draft, client_secret: secret, allowed_domains: domains.split(",").map(d => d.trim()).filter(Boolean) }),
      }, accessToken);
      if (!check) apply(result);
      showToast("success", check ? result.message ?? "Discovery checked." : "Provider settings saved.");
    } catch (error) { showToast("error", apiMessage(error, "Provider settings could not be checked or saved. Review the fields and try again.")); }
    finally { setAction(null); }
  }
  if (!overview) return failed ? <Button onClick={() => void load()}>Retry provider settings</Button> : <LoadingState label="Loading identity provider" />;
  return <section className="shared-settings-group">
    <PanelHeading icon={IconKey} title="Sign-in provider" description="One OpenID Connect provider for this server" />
    <p className="shared-muted-copy">Use a public HTTPS issuer with authorization code, S256 PKCE, RS256 and client_secret_post support. All endpoints must share its origin. Password sign-in stays available.</p>
    <p className="shared-muted-copy rm-identity-callback">Register this callback with the provider: <code>{overview.callback_url ?? "Set the server public URL first"}</code></p>
    <form ref={form} onSubmit={event => { event.preventDefault(); void submit(false); }}>
      <fieldset className="rm-identity-fields" disabled={busy}>
        <Field label="Provider name" required><Input maxLength={80} value={draft.name} onChange={event => edit({ name: event.target.value })} /></Field>
        <Field label="Issuer URL" required><Input type="url" value={draft.issuer} onChange={event => edit({ issuer: event.target.value })} /></Field>
        <Field label="Client ID" required><Input value={draft.client_id} onChange={event => edit({ client_id: event.target.value })} /></Field>
        <Field label="Client secret" hint={overview.has_secret ? "A secret is saved. Leave empty to keep it; enter a new one when changing the issuer or client ID." : "Stored encrypted. Never returned by the server."}><Input type="password" autoComplete="new-password" value={secret} onChange={event => { setSecret(event.target.value); setDirty(true); }} /></Field>
        <Field label="Allowed email domains" hint="Comma-separated exact domains. This does not grant workspace access or link an existing account."><Input value={domains} onChange={event => { setDomains(event.target.value); setDirty(true); }} /></Field>
        <Switch label="Enable provider" checked={draft.enabled} onCheckedChange={enabled => edit({ enabled })} />
        <div className="rm-identity-actions"><Button type="button" onClick={() => void submit(true)} disabled={busy} busy={action === "check"}>Check discovery</Button><Button type="submit" variant="main" busy={action === "save"}>Save provider</Button></div>
      </fieldset>
    </form>
  </section>;
}
