// Extends the System settings form: provider identity, callback, credentials, then
// check/save. Uses the existing neutral fields, loading state and toast feedback.
import { useEffect, useRef, useState } from "react";
import { IconKey } from "@tabler/icons-react";
import { request } from "../lib/sharedApi";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { Switch } from "./ui/switch";
import { LoadingState } from "./ui/loading-state";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { useUnsavedChanges } from "../lib/unsavedChanges";

interface Provider { name: string; issuer: string; client_id: string; enabled: boolean; allowed_domains: string[]; checked_at?: string | null; enforced?: boolean; group_claim?: string; group_mappings?: Mapping[] }
interface Mapping { group: string; scope: string; target_id: string; role: string }
interface Change { scope: string; target_id: string; role: string; group: string }
interface Plan { add: Change[]; change: Change[]; remove: Change[]; keep: Change[] }
interface Overview { provider: Provider | null; has_secret: boolean; callback_url: string | null; break_glass_emails?: string[] }
const empty: Provider = { name: "", issuer: "", client_id: "", enabled: false, allowed_domains: [], enforced: false };

export function IdentitySettings({ accessToken }: { accessToken: string }) {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [draft, setDraft] = useState(empty);
  const [secret, setSecret] = useState("");
  const [domains, setDomains] = useState("");
  const [breakGlass, setBreakGlass] = useState("");
  const [mappings, setMappings] = useState("");
  const [previewEmail, setPreviewEmail] = useState("");
  const [previewGroups, setPreviewGroups] = useState("");
  const [plan, setPlan] = useState<Plan | null>(null);
  const [action, setAction] = useState<"check" | "save" | null>(null);
  const busy = action !== null;
  const form = useRef<HTMLFormElement>(null);
  const [failed, setFailed] = useState(false);
  const [dirty, setDirty] = useState(false);
  useUnsavedChanges(dirty, { subject: "the identity provider settings" });
  function apply(value: Overview) { setOverview(value); setDraft(value.provider ?? empty); setDomains(value.provider?.allowed_domains.join(", ") ?? ""); setBreakGlass((value.break_glass_emails ?? []).join(", ")); setMappings((value.provider?.group_mappings ?? []).map(m => [m.group, m.scope, m.target_id, m.role].join(" | ")).join("\n")); setPlan(null); setSecret(""); setDirty(false); }
  async function load() {
    setFailed(false);
    try { apply(await request<Overview>("/v1/system/identity", {}, accessToken)); }
    catch (error) { setFailed(true); showToast("error", apiMessage(error, "Provider settings could not be loaded. Try again.")); }
  }
  useEffect(() => { void load(); }, [accessToken]);
  function edit(value: Partial<Provider>) { setDraft(current => ({ ...current, ...value })); setDirty(true); }
  function parseMappings(): Mapping[] {
    return mappings.split("\n").map(line => line.trim()).filter(Boolean).map(line => {
      const [group = "", scope = "", target_id = "", role = ""] = line.split("|").map(part => part.trim());
      return { group, scope, target_id, role };
    });
  }
  async function preview() {
    setAction("check");
    try {
      const result = await request<{ plan: Plan }>("/v1/system/identity/groups/preview", { method: "POST", body: JSON.stringify({ email: previewEmail, groups: previewGroups.split(",").map(g => g.trim()).filter(Boolean), group_mappings: parseMappings() }) }, accessToken);
      setPlan(result.plan);
    } catch (error) { showToast("error", apiMessage(error, "The group mapping preview failed. Check the mapping lines and the email address.")); }
    finally { setAction(null); }
  }
  async function submit(check: boolean) {
    if (!form.current?.reportValidity()) return;
    setAction(check ? "check" : "save");
    try {
      const result = await request<Overview & { message?: string }>(`/v1/system/identity${check ? "/check" : ""}`, {
        method: check ? "POST" : "PUT", body: JSON.stringify({ ...draft, client_secret: secret, allowed_domains: domains.split(",").map(d => d.trim()).filter(Boolean), break_glass_emails: breakGlass.split(",").map(d => d.trim()).filter(Boolean), group_mappings: parseMappings() }),
      }, accessToken);
      if (!check) apply(result);
      showToast("success", check ? result.message ?? "Discovery checked." : "Provider settings saved.");
    } catch (error) { showToast("error", apiMessage(error, "Provider settings could not be checked or saved. Review the fields and try again.")); }
    finally { setAction(null); }
  }
  if (!overview) return failed ? <Button onClick={() => void load()}>Retry provider settings</Button> : <LoadingState label="Loading identity provider" />;
  return <section className="shared-settings-group">
    <PanelHeading icon={IconKey} title="Sign-in provider" description="One OpenID Connect provider for this server" />
    <p className="shared-muted-copy">Use a public HTTPS issuer with authorization code, S256 PKCE, RS256 and client_secret_post support. All endpoints must share its origin. Password sign-in stays available unless you require the provider below.</p>
    <p className="shared-muted-copy rm-identity-callback">Register this callback with the provider: <code>{overview.callback_url ?? "Set the server public URL first"}</code></p>
    <form ref={form} onSubmit={event => { event.preventDefault(); void submit(false); }}>
      <fieldset className="rm-identity-fields" disabled={busy}>
        <Field label="Provider name" required><Input maxLength={80} value={draft.name} onChange={event => edit({ name: event.target.value })} /></Field>
        <Field label="Issuer URL" required><Input type="url" value={draft.issuer} onChange={event => edit({ issuer: event.target.value })} /></Field>
        <Field label="Client ID" required><Input value={draft.client_id} onChange={event => edit({ client_id: event.target.value })} /></Field>
        <Field label="Client secret" hint={overview.has_secret ? "A secret is saved. Leave empty to keep it; enter a new one when changing the issuer or client ID." : "Stored encrypted. Never returned by the server."}><Input type="password" autoComplete="new-password" value={secret} onChange={event => { setSecret(event.target.value); setDirty(true); }} /></Field>
        <Field label="Allowed email domains" hint="Comma-separated exact domains. This does not grant workspace access or link an existing account."><Input value={domains} onChange={event => { setDomains(event.target.value); setDirty(true); }} /></Field>
        <Switch label="Enable provider" checked={draft.enabled} onCheckedChange={enabled => edit({ enabled, ...(enabled ? {} : { enforced: false }) })} />
        <Switch label="Require provider sign-in" checked={Boolean(draft.enforced)} disabled={!draft.enabled} onCheckedChange={enforced => edit({ enforced })} />
        {draft.enforced ? <Field label="Break-glass administrators" required hint="One to five active system administrators, comma-separated, who keep password sign-in. You must have linked your own account to this provider first. Each use is recorded in the audit trail."><Input required value={breakGlass} onChange={event => { setBreakGlass(event.target.value); setDirty(true); }} /></Field> : null}
        <Field label="Group claim" hint="The ID-token claim that lists the person's groups. Leave empty for “groups”. A token without it changes no memberships."><Input maxLength={64} value={draft.group_claim ?? ""} onChange={event => edit({ group_claim: event.target.value })} /></Field>
        <Field label="Group mappings" hint="One per line: group | organization or workspace | target ID | role. Roles: admin or member for an organization; admin, member or viewer for a workspace. Ownership is never mapped, and memberships set directly are never changed or removed."><Textarea rows={4} value={mappings} onChange={event => { setMappings(event.target.value); setDirty(true); }} /></Field>
        <Field label="Preview for a person" hint="Shows what signing in with these groups would add, change or remove, without doing it."><Input type="email" placeholder="person@example.com" value={previewEmail} onChange={event => setPreviewEmail(event.target.value)} /></Field>
        <Field label="Their provider groups" hint="Comma-separated."><Input value={previewGroups} onChange={event => setPreviewGroups(event.target.value)} /></Field>
        <Button type="button" onClick={() => void preview()} disabled={busy || !previewEmail}>Preview group changes</Button>
        {plan ? <ul aria-label="Group mapping preview">{(["add", "change", "remove", "keep"] as const).flatMap(kind => plan[kind].map(c => <li key={`${kind}-${c.scope}-${c.target_id}`}>{kind === "keep" ? "Keep (already held directly)" : kind[0].toUpperCase() + kind.slice(1)}: {c.scope} {c.target_id} as {c.role} via {c.group}</li>))}{Object.values(plan).every(list => list.length === 0) ? <li>No membership changes.</li> : null}</ul> : null}
        <div className="rm-identity-actions"><Button type="button" onClick={() => void submit(true)} disabled={busy} busy={action === "check"}>Check discovery</Button><Button type="submit" variant="main" busy={action === "save"}>Save provider</Button></div>
      </fieldset>
    </form>
  </section>;
}
