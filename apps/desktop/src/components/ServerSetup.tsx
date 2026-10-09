import { useEffect, useMemo, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import {
  IconAlertCircle as AlertCircle,
  IconAlertTriangle as AlertTriangle,
  IconArrowLeft as ArrowLeft,
  IconArrowRight as ArrowRight,
  IconBuilding as Building,
  IconCircleCheck as CircleCheck,
  IconFileText as FileText,
  IconFlag as Flag,
  IconInfoCircle as InfoCircle,
  IconKey as Key,
  IconLoader2 as Loader,
  IconLock as Lock,
  IconLogout as Logout,
  IconRefresh as Refresh,
  IconServer as Server,
  IconShieldLock as ShieldLock,
  IconUserShield as UserShield,
} from "@tabler/icons-react";
import {
  completeServerSetup,
  createSetupAdmin,
  createSharedOrganization,
  createSharedWorkspace,
  getSetupChecks,
  getSystemSettings,
  saveSystemSettings,
  SharedApiError,
  verifySetupCode,
} from "../lib/sharedApi";
import type { SetupCheck, SharedSession, SystemSetting } from "../types";
import { ThemeToggle } from "./SharedLayout";
import { Button } from "./ui/button";
import { Dropdown } from "./ui/dropdown";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";

type StepKey = "welcome" | "admin" | "checks" | "access" | "logging" | "organization" | "finish";

const STEPS: { key: StepKey; label: string; icon: ReactNode }[] = [
  { key: "welcome", label: "Welcome", icon: <Key size={22} /> },
  { key: "admin", label: "System administrator", icon: <UserShield size={22} /> },
  { key: "checks", label: "Server check", icon: <Server size={22} /> },
  { key: "access", label: "Access and security", icon: <ShieldLock size={22} /> },
  { key: "logging", label: "Logging", icon: <FileText size={22} /> },
  { key: "organization", label: "First workspace", icon: <Building size={22} /> },
  { key: "finish", label: "Finish", icon: <Flag size={22} /> },
];

/** Settings offered during setup; everything else stays at the server default and can be changed later in System › Settings. */
const ACCESS_SETTINGS = ["allow_registration", "refresh_token_ttl_days", "login_max_failures", "login_lockout_minutes", "ai_requests_per_hour"];
const LOGGING_SETTINGS = ["log_level", "log_to_file", "log_retention_days", "log_console_format"];

type SettingValue = boolean | number | string;

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

/**
 * The onboarding of a brand-new server. In the `new` phase anyone who reaches the server sees the first two steps, and
 * the setup code from the server console is needed to create the system administrator. In the `finishing` phase only
 * that administrator, signed in, continues. The server decides which phase applies and refuses every setup request
 * once setup is complete, so this page cannot be used again.
 */
export function ServerSetup({ accessToken, onAdminCreated, onFinished, onSignOut, phase, session }: {
  accessToken?: string;
  onAdminCreated?: (accessToken: string) => void;
  onFinished?: () => void;
  onSignOut?: () => void;
  phase: "new" | "finishing";
  session?: SharedSession;
}) {
  const [step, setStep] = useState<StepKey>(phase === "new" ? "welcome" : "checks");
  const [code, setCode] = useState("");
  const [settings, setSettings] = useState<SystemSetting[] | null>(null);
  const [changedSettings, setChangedSettings] = useState<string[]>([]);
  const [created, setCreated] = useState<{ organization: string; workspace: string | null } | null>(null);
  const [warnings, setWarnings] = useState(0);
  const stepIndex = STEPS.findIndex((entry) => entry.key === step);
  const firstFinishingStep = STEPS.findIndex((entry) => entry.key === "checks");

  useEffect(() => {
    if (phase !== "finishing" || !accessToken) return;
    getSystemSettings(accessToken)
      .then((response) => setSettings(response.settings))
      .catch((error) => showToast("error", errorMessage(error, "The server settings could not be loaded.")));
  }, [phase, accessToken]);

  const next = () => setStep(STEPS[Math.min(stepIndex + 1, STEPS.length - 1)].key);
  const back = () => setStep(STEPS[Math.max(stepIndex - 1, phase === "finishing" ? firstFinishingStep : 0)].key);

  return <main className="rm-setup">
    <header className="rm-setup-header">
      <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
      <div className="rm-setup-header-actions">
        {session ? <span className="rm-setup-signed-in">Signed in as {session.user.email ?? session.user.display_name}</span> : null}
        {session && onSignOut ? <Button onClick={onSignOut} type="button" variant="secondary"><Logout size={15} /> Sign out</Button> : null}
        <ThemeToggle />
      </div>
    </header>
    <div className="rm-setup-body">
      <nav aria-label="Setup steps" className="rm-setup-nav">
        <p className="shared-eyebrow">Server setup</p>
        <ol className="rm-setup-steps">
          {STEPS.map((entry, index) => {
            const state = index < stepIndex ? "done" : index === stepIndex ? "current" : "upcoming";
            return <li aria-current={state === "current" ? "step" : undefined} className={`rm-setup-step ${state}`} key={entry.key}>
              <span className="rm-setup-step-mark" aria-hidden="true">{state === "done" ? <CircleCheck size={22} /> : entry.icon}</span>
              <span>{entry.label}<small>{state === "done" ? "Done" : state === "current" ? "In progress" : "To do"}</small></span>
            </li>;
          })}
        </ol>
        <p className="rm-setup-nav-note"><Lock size={14} /> This page is only available until the setup is finished. It never shows again afterwards.</p>
      </nav>
      <section className="rm-setup-panel" aria-live="polite">
        {step === "welcome" ? <WelcomeStep code={code} onCode={setCode} onVerified={next} /> : null}
        {step === "admin" ? <AdminStep code={code} onBack={back} onCodeRejected={() => setStep("welcome")} onCreated={(token) => onAdminCreated?.(token)} /> : null}
        {step === "checks" && accessToken ? <ChecksStep accessToken={accessToken} onNext={next} onWarnings={setWarnings} /> : null}
        {step === "access" && accessToken ? <SettingsStep accessToken={accessToken} description="Who can join this server and how sign-in is protected. These and many more settings stay available in System › Settings." keys={ACCESS_SETTINGS} onBack={back} onNext={next} onSaved={(keys, saved) => { setSettings(saved); setChangedSettings((current) => [...new Set([...current, ...keys])]); }} settings={settings} title="Access and security" /> : null}
        {step === "logging" && accessToken ? <SettingsStep accessToken={accessToken} description="What the server records and how long log files are kept. Each kind of log can be fine-tuned later in System › Settings › Logging." keys={LOGGING_SETTINGS} onBack={back} onNext={next} onSaved={(keys, saved) => { setSettings(saved); setChangedSettings((current) => [...new Set([...current, ...keys])]); }} settings={settings} title="Logging" /> : null}
        {step === "organization" && accessToken ? <OrganizationStep accessToken={accessToken} created={created} onBack={back} onCreated={setCreated} onNext={next} /> : null}
        {step === "finish" && accessToken ? <FinishStep accessToken={accessToken} changedSettings={(settings ?? []).filter((setting) => changedSettings.includes(setting.key))} created={created} onBack={back} onFinished={() => onFinished?.()} session={session} warnings={warnings} /> : null}
      </section>
    </div>
  </main>;
}

function StepHeading({ eyebrow, title, children }: { eyebrow: string; title: string; children: ReactNode }) {
  return <div className="shared-auth-heading rm-setup-heading"><p className="shared-eyebrow">{eyebrow}</p><h2>{title}</h2><p>{children}</p></div>;
}

function WelcomeStep({ code, onCode, onVerified }: { code: string; onCode: (code: string) => void; onVerified: () => void }) {
  const [isBusy, setIsBusy] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true);
    try { await verifySetupCode(code); onVerified(); }
    catch (error) { showToast("error", errorMessage(error, "The setup code could not be checked.")); }
    finally { setIsBusy(false); }
  }
  return <>
    <StepHeading eyebrow="Step 1 of 7" title="Set up this RepoMemo server">
      This server has no account yet. You will create its system administrator, check the server, choose the first settings and, if you like, a first workspace.
    </StepHeading>
    <ul className="rm-setup-points">
      <li><ShieldLock size={16} /> Only someone with access to the server can do this: enter the one-time setup code printed on the server console when it started (or the value of <code>REPOMEMO_SETUP_CODE</code>).</li>
      <li><UserShield size={16} /> The account you create administers the whole server: every organization and workspace, its users and its settings.</li>
      <li><Lock size={16} /> Until setup is finished, nobody else can create an account here.</li>
    </ul>
    <form className="shared-setup-form" onSubmit={submit}>
      <label>Setup code<Input autoComplete="one-time-code" autoFocus onChange={(event) => onCode(event.target.value)} placeholder="ABCD-EFGH-JKLM" required spellCheck={false} value={code} /></label>
      <div className="rm-setup-actions"><span /><Button disabled={isBusy || !code.trim()} type="submit" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <ArrowRight size={16} />} Continue</Button></div>
    </form>
  </>;
}

function AdminStep({ code, onBack, onCodeRejected, onCreated }: { code: string; onBack: () => void; onCodeRejected: () => void; onCreated: (accessToken: string) => void }) {
  const [displayName, setDisplayName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (password !== confirmation) { showToast("error", "The two passwords do not match."); return; }
    setIsBusy(true);
    try {
      const response = await createSetupAdmin({ code, email, displayName, password });
      showToast("success", "System administrator created. You are signed in.");
      onCreated(response.access_token);
    } catch (error) {
      showToast("error", errorMessage(error, "The administrator could not be created."));
      // The code changes when the server restarts; ask for it again.
      if (error instanceof SharedApiError && error.status === 403) onCodeRejected();
    } finally { setIsBusy(false); }
  }
  return <>
    <StepHeading eyebrow="Step 2 of 7" title="Create the system administrator">
      Use an address you keep long-term and a strong password: this account can change everything on the server.
    </StepHeading>
    <form className="shared-setup-form" onSubmit={submit}>
      <label>Name<Input autoComplete="name" autoFocus maxLength={120} onChange={(event) => setDisplayName(event.target.value)} placeholder="Ada Lovelace" required value={displayName} /></label>
      <label>Email<Input autoComplete="email" onChange={(event) => setEmail(event.target.value)} placeholder="admin@company.com" required type="email" value={email} /></label>
      <label>Password<Input autoComplete="new-password" minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={password} /></label>
      <label>Confirm password<Input autoComplete="new-password" minLength={12} onChange={(event) => setConfirmation(event.target.value)} required type="password" value={confirmation} /></label>
      <div className="rm-setup-actions"><Button disabled={isBusy} onClick={onBack} type="button" variant="secondary"><ArrowLeft size={16} /> Back</Button><Button disabled={isBusy} type="submit" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <UserShield size={16} />} Create administrator</Button></div>
    </form>
  </>;
}

const CHECK_ICONS = { ok: CircleCheck, info: InfoCircle, warning: AlertTriangle, error: AlertCircle } as const;
const CHECK_LABELS = { ok: "OK", info: "Note", warning: "Attention", error: "Problem" } as const;

function ChecksStep({ accessToken, onNext, onWarnings }: { accessToken: string; onNext: () => void; onWarnings: (count: number) => void }) {
  const [checks, setChecks] = useState<SetupCheck[] | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  async function run() {
    setIsBusy(true);
    try {
      const next = await getSetupChecks(accessToken);
      setChecks(next);
      onWarnings(next.filter((check) => check.status === "warning" || check.status === "error").length);
    } catch (error) { showToast("error", errorMessage(error, "The server could not be checked.")); }
    finally { setIsBusy(false); }
  }
  useEffect(() => { void run(); }, [accessToken]);
  const problems = checks?.filter((check) => check.status === "error").length ?? 0;
  return <>
    <StepHeading eyebrow="Step 3 of 7" title="Check the server">
      What the server found about its own setup. Warnings do not block anything; they are things to fix in the server's environment before many people use it.
    </StepHeading>
    {checks ? <ul className="rm-setup-checks">{checks.map((check) => {
      const Icon = CHECK_ICONS[check.status];
      return <li className={`rm-setup-check status-${check.status}`} key={check.key}>
        <Icon aria-hidden="true" className="rm-setup-check-icon" size={18} />
        <div><strong>{check.label}<small>{CHECK_LABELS[check.status]}</small></strong><p>{check.detail}</p>{check.advice && check.status !== "ok" ? <p className="rm-setup-advice">{check.advice}</p> : null}</div>
      </li>;
    })}</ul> : <p className="shared-muted-copy"><Loader className="spin" size={14} /> Checking the server…</p>}
    <div className="rm-setup-actions">
      <Button disabled={isBusy} onClick={() => void run()} type="button" variant="secondary">{isBusy ? <Loader className="spin" size={16} /> : <Refresh size={16} />} Check again</Button>
      <Button disabled={!checks} onClick={onNext} type="button" variant="main"><ArrowRight size={16} /> {problems ? "Continue anyway" : "Continue"}</Button>
    </div>
  </>;
}

function settingValueLabel(setting: SystemSetting, value: SettingValue): string {
  if (setting.kind === "boolean") return value ? "On" : "Off";
  if (setting.kind === "choice") return setting.options.find((option) => option.value === value)?.label ?? String(value);
  return `${value}${setting.unit ? ` ${setting.unit}` : ""}`;
}

function SettingsStep({ accessToken, description, keys, onBack, onNext, onSaved, settings, title }: {
  accessToken: string;
  description: string;
  keys: string[];
  onBack: () => void;
  onNext: () => void;
  onSaved: (keys: string[], settings: SystemSetting[]) => void;
  settings: SystemSetting[] | null;
  title: string;
}) {
  const [draft, setDraft] = useState<Record<string, SettingValue>>({});
  const [isBusy, setIsBusy] = useState(false);
  const shown = useMemo(() => keys.map((key) => settings?.find((setting) => setting.key === key)).filter((setting): setting is SystemSetting => Boolean(setting)), [keys, settings]);
  const changed = Object.keys(draft);

  function edit(setting: SystemSetting, value: SettingValue) {
    setDraft((current) => {
      const next = { ...current };
      if (value === setting.value) delete next[setting.key];
      else next[setting.key] = value;
      return next;
    });
  }

  async function save() {
    if (!changed.length) { onNext(); return; }
    setIsBusy(true);
    try {
      const response = await saveSystemSettings(accessToken, draft);
      onSaved(changed, response.settings);
      setDraft({});
      onNext();
    } catch (error) { showToast("error", errorMessage(error, "The settings could not be saved.")); }
    finally { setIsBusy(false); }
  }

  const position = title === "Logging" ? 5 : 4;
  return <>
    <StepHeading eyebrow={`Step ${position} of 7`} title={title}>{description}</StepHeading>
    {settings ? <div className="rm-system-settings">{shown.map((setting) => {
      const value = draft[setting.key] ?? setting.value;
      const inputId = `setup-setting-${setting.key}`;
      return <div className="rm-system-setting" key={setting.key}>
        <div><label htmlFor={inputId}><strong>{setting.label}</strong></label><p>{setting.description}</p><small>Server default: {settingValueLabel(setting, setting.default_value)}</small></div>
        <div className="rm-system-setting-control">
          {setting.kind === "boolean"
            ? <label className="shared-ai-provider-toggle"><input checked={Boolean(value)} id={inputId} onChange={(event) => edit(setting, event.target.checked)} type="checkbox" /> {value ? "On" : "Off"}</label>
            : setting.kind === "choice"
              ? <Dropdown aria-label={setting.label} id={inputId} onValueChange={(next) => edit(setting, next)} options={setting.options} value={String(value)} />
              : <span className="rm-system-number"><Input id={inputId} max={setting.max} min={setting.min} onChange={(event) => edit(setting, event.target.value === "" ? setting.min : Number(event.target.value))} type="number" value={String(value)} /><small>{setting.unit}</small></span>}
        </div>
      </div>;
    })}</div> : <p className="shared-muted-copy"><Loader className="spin" size={14} /> Loading the settings…</p>}
    <div className="rm-setup-actions">
      <Button disabled={isBusy} onClick={onBack} type="button" variant="secondary"><ArrowLeft size={16} /> Back</Button>
      <Button disabled={isBusy || !settings} onClick={() => void save()} type="button" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <ArrowRight size={16} />} {changed.length ? "Save and continue" : "Keep these and continue"}</Button>
    </div>
  </>;
}

function OrganizationStep({ accessToken, created, onBack, onCreated, onNext }: {
  accessToken: string;
  created: { organization: string; workspace: string | null } | null;
  onBack: () => void;
  onCreated: (created: { organization: string; workspace: string | null }) => void;
  onNext: () => void;
}) {
  const [organization, setOrganization] = useState("");
  const [workspace, setWorkspace] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true);
    try {
      const createdOrganization = await createSharedOrganization(accessToken, organization.trim());
      const createdWorkspace = workspace.trim() ? await createSharedWorkspace(accessToken, createdOrganization.id, workspace.trim()) : null;
      onCreated({ organization: createdOrganization.name, workspace: createdWorkspace?.workspace.name ?? null });
      onNext();
    } catch (error) { showToast("error", errorMessage(error, "The organization could not be created.")); }
    finally { setIsBusy(false); }
  }
  return <>
    <StepHeading eyebrow="Step 6 of 7" title="Create a first workspace">
      Optional. An organization is a team or company; a workspace holds one body of knowledge, such as a project or a service. You own what you create here and can invite people afterwards.
    </StepHeading>
    {created ? <p className="rm-setup-created"><CircleCheck size={16} /> Created {created.organization}{created.workspace ? ` with the workspace ${created.workspace}` : ""}.</p> : null}
    {created ? <div className="rm-setup-actions"><Button onClick={onBack} type="button" variant="secondary"><ArrowLeft size={16} /> Back</Button><Button onClick={onNext} type="button" variant="main"><ArrowRight size={16} /> Continue</Button></div> : <form className="shared-setup-form" onSubmit={submit}>
      <label>Organization name<Input maxLength={120} onChange={(event) => setOrganization(event.target.value)} placeholder="Acme Engineering" required value={organization} /></label>
      <label>Workspace name (optional)<Input maxLength={120} onChange={(event) => setWorkspace(event.target.value)} placeholder="Payments service" value={workspace} /></label>
      <div className="rm-setup-actions">
        <Button disabled={isBusy} onClick={onBack} type="button" variant="secondary"><ArrowLeft size={16} /> Back</Button>
        <span className="rm-setup-actions-end"><Button disabled={isBusy} onClick={onNext} type="button" variant="secondary">Skip</Button><Button disabled={isBusy || !organization.trim()} type="submit" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <Building size={16} />} Create and continue</Button></span>
      </div>
    </form>}
  </>;
}

function FinishStep({ accessToken, changedSettings, created, onBack, onFinished, session, warnings }: {
  accessToken: string;
  changedSettings: SystemSetting[];
  created: { organization: string; workspace: string | null } | null;
  onBack: () => void;
  onFinished: () => void;
  session?: SharedSession;
  warnings: number;
}) {
  const [isBusy, setIsBusy] = useState(false);
  async function finish() {
    setIsBusy(true);
    try {
      await completeServerSetup(accessToken);
      showToast("success", "The server is set up. Welcome to RepoMemo.");
      onFinished();
    } catch (error) { showToast("error", errorMessage(error, "The setup could not be finished.")); setIsBusy(false); }
  }
  return <>
    <StepHeading eyebrow="Step 7 of 7" title="Review and finish">
      Finishing opens the server to the people you invite, under the settings above. This setup page then disappears for good; everything can still be changed in System.
    </StepHeading>
    <dl className="rm-system-facts">
      <div><dt>System administrator</dt><dd>{session?.user.email ?? session?.user.display_name ?? "You"}</dd></div>
      <div><dt>Server check</dt><dd>{warnings ? `${warnings} point${warnings === 1 ? "" : "s"} to look at` : "No warnings"}</dd></div>
      <div><dt>Settings changed</dt><dd>{changedSettings.length ? changedSettings.map((setting) => `${setting.label}: ${settingValueLabel(setting, setting.value)}`).join(" · ") : "Server defaults kept"}</dd></div>
      <div><dt>First workspace</dt><dd>{created ? `${created.organization}${created.workspace ? ` / ${created.workspace}` : ""}` : "Skipped"}</dd></div>
    </dl>
    <div className="rm-setup-actions">
      <Button disabled={isBusy} onClick={onBack} type="button" variant="secondary"><ArrowLeft size={16} /> Back</Button>
      <Button disabled={isBusy} onClick={() => void finish()} type="button" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <Flag size={16} />} Finish setup</Button>
    </div>
  </>;
}

/** Shown to a signed-in account that is not the system administrator while the server is still being set up. */
export function ServerSetupWaiting({ onSignOut }: { onSignOut: () => void }) {
  return <main className="rm-setup rm-setup-waiting">
    <section className="rm-setup-panel">
      <StepHeading eyebrow="Server setup" title="This server is being set up">
        Its system administrator has not finished the setup yet. Try again later, or sign in as the system administrator.
      </StepHeading>
      <div className="rm-setup-actions"><span /><Button onClick={onSignOut} type="button" variant="secondary"><Logout size={16} /> Sign out</Button></div>
    </section>
  </main>;
}
