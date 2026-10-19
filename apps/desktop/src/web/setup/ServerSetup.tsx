import { useEffect, useState } from "react";
import { IconLogout as Logout } from "@tabler/icons-react";
import { getSystemSettings } from "../../lib/sharedApi";
import type { SharedSession, SystemSetting } from "../../types";
import { Button } from "../../components/ui/button";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";
import { type StepKey, STEPS, ACCESS_SETTINGS, LOGGING_SETTINGS } from "./steps";
import { StepFrame, StepHeading } from "./StepFrame";
import { WelcomeStep } from "./WelcomeStep";
import { AdminStep } from "./AdminStep";
import { ChecksStep } from "./ChecksStep";
import { SettingsStep } from "./SettingsStep";
import { OrganizationStep } from "./OrganizationStep";
import { FinishStep } from "./FinishStep";

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
      .catch((error) => showToast("error", apiMessage(error, "The server settings could not be loaded.")));
  }, [phase, accessToken]);

  const next = () => setStep(STEPS[Math.min(stepIndex + 1, STEPS.length - 1)].key);
  const back = () => setStep(STEPS[Math.max(stepIndex - 1, phase === "finishing" ? firstFinishingStep : 0)].key);

  const settingsSaved = (keys: string[], saved: SystemSetting[]) => {
    setSettings(saved);
    setChangedSettings((current) => [...new Set([...current, ...keys])]);
  };

  return <StepFrame onSignOut={onSignOut} session={session} stepIndex={stepIndex}>
    {step === "welcome" ? <WelcomeStep code={code} onCode={setCode} onVerified={next} /> : null}
    {step === "admin" ? <AdminStep code={code} onBack={back} onCodeRejected={() => setStep("welcome")} onCreated={(token) => onAdminCreated?.(token)} /> : null}
    {step === "checks" && accessToken ? <ChecksStep accessToken={accessToken} onNext={next} onWarnings={setWarnings} /> : null}
    {step === "access" && accessToken ? <SettingsStep accessToken={accessToken} description="Who can join this server and how sign-in is protected. These and many more settings stay available in System › Settings." keys={ACCESS_SETTINGS} onBack={back} onNext={next} onSaved={settingsSaved} settings={settings} title="Access and security" /> : null}
    {step === "logging" && accessToken ? <SettingsStep accessToken={accessToken} description="What the server records and how long log files are kept. Each kind of log can be fine-tuned later in System › Settings › Logging." keys={LOGGING_SETTINGS} onBack={back} onNext={next} onSaved={settingsSaved} settings={settings} title="Logging" /> : null}
    {step === "organization" && accessToken ? <OrganizationStep accessToken={accessToken} created={created} onBack={back} onCreated={setCreated} onNext={next} /> : null}
    {step === "finish" && accessToken ? <FinishStep accessToken={accessToken} changedSettings={(settings ?? []).filter((setting) => changedSettings.includes(setting.key))} created={created} onBack={back} onFinished={() => onFinished?.()} session={session} warnings={warnings} /> : null}
  </StepFrame>;
}

/** Shown to a signed-in account that is not the system administrator while the server is still being set up. */
export function ServerSetupWaiting({ onSignOut }: { onSignOut: () => void }) {
  return <main className="rm-setup rm-setup-waiting">
    <section className="rm-setup-panel">
      <StepHeading eyebrow="Server setup" title="This server is being set up">
        Its system administrator has not finished the setup yet. Try again later, or sign in as the system administrator.
      </StepHeading>
      <div className="rm-setup-actions"><span /><Button onClick={onSignOut} type="button" variant="secondary"><Logout size={ICON_SIZE.md} /> Sign out</Button></div>
    </section>
  </main>;
}
