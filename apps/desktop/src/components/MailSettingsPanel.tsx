import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { IconMail as Mail, IconSend as Send } from "@tabler/icons-react";
import { getSystemMail, saveSystemMail, sendSystemTestMail } from "../lib/sharedApi";
import type { MailOverview } from "../types";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { EmptyState } from "./ui/empty-state";
import { Field, FieldRow } from "./ui/field";
import { LoadingState } from "./ui/loading-state";
import { PanelHeading } from "./ui/panel-heading";
import { Dropdown } from "./ui/dropdown";
import { Input } from "./ui/input";
import { Switch } from "./ui/switch";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

function when(value: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

const STATUS_LABEL: Record<string, string> = { queued: "Waiting", sending: "Sending", sent: "Sent", failed: "Failed" };

/**
 * System › Settings › Mail: the mail server the server sends through, a test
 * message and the outbox. The password is never shown: only whether one is
 * saved; leave the field empty to keep it.
 */
export function MailSettingsPanel({ accessToken, canChange }: { accessToken: string; canChange: boolean }) {
  const [overview, setOverview] = useState<MailOverview | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [host, setHost] = useState("");
  const [port, setPort] = useState("587");
  const [security, setSecurity] = useState("starttls");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [removePassword, setRemovePassword] = useState(false);
  const [fromAddress, setFromAddress] = useState("");
  const [fromName, setFromName] = useState("");
  const [testTo, setTestTo] = useState("");
  const [action, setAction] = useState<"save" | "test" | null>(null);
  const isBusy = action !== null;

  function show(next: MailOverview) {
    setOverview(next);
    const settings = next.settings;
    setEnabled(settings.enabled);
    setHost(settings.host);
    setPort(String(settings.port));
    setSecurity(settings.security);
    setUsername(settings.username);
    setFromAddress(settings.from_address);
    setFromName(settings.from_name);
    setPassword("");
    setRemovePassword(false);
  }

  async function load() {
    try { show(await getSystemMail(accessToken)); } catch (error) { showToast("error", apiMessage(error, "The mail settings could not be loaded.")); }
  }

  useEffect(() => { void load(); }, [accessToken]);
  // The outbox moves on its own while something is waiting.
  useEffect(() => {
    if (!overview || overview.queued_count === 0) return;
    const timer = window.setTimeout(() => void load(), 4000);
    return () => window.clearTimeout(timer);
  }, [overview]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAction("save");
    try {
      show(await saveSystemMail(accessToken, {
        enabled, host, port: Number(port), security, username, from_address: fromAddress, from_name: fromName,
        ...(removePassword ? { password: "" } : password ? { password } : {}),
      }));
      showToast("success", "Mail settings saved.");
    } catch (error) { showToast("error", apiMessage(error, "The mail settings could not be saved.")); } finally { setAction(null); }
  }

  async function sendTest() {
    setAction("test");
    try {
      await sendSystemTestMail(accessToken, testTo.trim() || undefined);
      showToast("success", "Test message queued. Its result shows in the list below.");
      await load();
    } catch (error) { showToast("error", apiMessage(error, "The test message could not be queued.")); } finally { setAction(null); }
  }

  if (!overview) return <LoadingState label="Loading mail settings" />;
  const settings = overview.settings;
  const readOnly = !canChange;
  return <section className="shared-settings-group">
    <PanelHeading description={settings.enabled ? "on" : settings.configured ? "off" : "not set up"} icon={Mail} title="Mail" />
    <p className="shared-muted-copy">The server sends invitations and notices through your mail server over TLS. Mail is never sent in plain text. Servers on a private address need the operator's say (<code>REPOMEMO_SMTP_ALLOWED_HOSTS</code>).</p>
    <form className="rm-mail-form" onSubmit={save}>
      <Switch checked={enabled} disabled={readOnly} label="Send mail" onCheckedChange={setEnabled} />
      <Field label="Mail server"><Input disabled={readOnly} onChange={(event) => setHost(event.target.value)} placeholder="smtp.example.com" value={host} /></Field>
      <FieldRow>
        <Field label="Connection">{(control) => <Dropdown disabled={readOnly} id={control.id} onValueChange={(value) => { setSecurity(value); setPort(value === "tls" ? "465" : "587"); }} options={[{ label: "STARTTLS (usually port 587)", value: "starttls" }, { label: "TLS from the start (usually port 465)", value: "tls" }]} value={security} />}</Field>
        <Field label="Port"><Input disabled={readOnly} inputMode="numeric" onChange={(event) => setPort(event.target.value)} value={port} /></Field>
      </FieldRow>
      <FieldRow>
        <Field label="User name"><Input autoComplete="off" disabled={readOnly} onChange={(event) => setUsername(event.target.value)} value={username} /></Field>
        <Field label="Password"><Input autoComplete="new-password" disabled={readOnly || removePassword} onChange={(event) => setPassword(event.target.value)} placeholder={settings.password_set ? "Saved: leave empty to keep it" : "Not set"} type="password" value={password} /></Field>
      </FieldRow>
      {settings.password_set && !readOnly ? <Checkbox checked={removePassword} label="Remove the saved password" onCheckedChange={setRemovePassword} /> : null}
      <FieldRow>
        <Field label="Sender address"><Input disabled={readOnly} onChange={(event) => setFromAddress(event.target.value)} placeholder="repomemo@example.com" type="email" value={fromAddress} /></Field>
        <Field label="Sender name"><Input disabled={readOnly} onChange={(event) => setFromName(event.target.value)} placeholder="RepoMemo" value={fromName} /></Field>
      </FieldRow>
      {readOnly ? null : <div><Button busy={action === "save"} disabled={isBusy} type="submit" variant="main">Save mail settings</Button></div>}
    </form>
    {readOnly ? null : <div className="rm-mail-test">
      <Field label="Send the test message to"><Input onChange={(event) => setTestTo(event.target.value)} placeholder="Your own address when empty" type="email" value={testTo} /></Field>
      <Button busy={action === "test"} disabled={isBusy || !settings.configured || !settings.enabled} onClick={() => void sendTest()} variant="secondary"><Send size={ICON_SIZE.sm} /> Send test mail</Button>
    </div>}
    <p className="shared-muted-copy">{overview.queued_count} waiting · {overview.sent_last_day} sent and {overview.failed_last_day} failed in the last day</p>
    {overview.entries.length ? <div className="shared-activity-list">{overview.entries.map((entry) => <article key={entry.id}>
      <div><strong>{entry.subject}</strong><span>{entry.to_address} · {entry.kind} · {STATUS_LABEL[entry.status] ?? entry.status}{entry.attempts ? ` · ${entry.attempts} ${entry.attempts === 1 ? "try" : "tries"}` : ""}{entry.last_error ? ` · ${entry.last_error}` : ""}</span></div>
      <time dateTime={entry.sent_at ?? entry.created_at}>{when(entry.sent_at ?? entry.created_at)}</time>
    </article>)}</div> : <EmptyState title="No messages yet" />}
  </section>;
}
