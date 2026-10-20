import { useEffect, useState } from "react";
import {
  IconAlertCircle as AlertCircle,
  IconAlertTriangle as AlertTriangle,
  IconArrowRight as ArrowRight,
  IconCircleCheck as CircleCheck,
  IconInfoCircle as InfoCircle,
  IconRefresh as Refresh,
} from "@tabler/icons-react";
import { getSetupChecks } from "../../lib/sharedApi";
import type { SetupCheck } from "../../types";
import { Button } from "../../components/ui/button";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { LoadingState } from "../../components/ui/loading-state";
import { apiMessage } from "../format";
import { StepHeading } from "./StepFrame";

const CHECK_ICONS = { ok: CircleCheck, info: InfoCircle, warning: AlertTriangle, error: AlertCircle } as const;
const CHECK_LABELS = { ok: "OK", info: "Note", warning: "Attention", error: "Problem" } as const;

export function ChecksStep({ accessToken, onNext, onWarnings }: { accessToken: string; onNext: () => void; onWarnings: (count: number) => void }) {
  const [checks, setChecks] = useState<SetupCheck[] | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  async function run() {
    setIsBusy(true);
    try {
      const next = await getSetupChecks(accessToken);
      setChecks(next);
      onWarnings(next.filter((check) => check.status === "warning" || check.status === "error").length);
    } catch (error) { showToast("error", apiMessage(error, "The server could not be checked.")); }
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
        <Icon aria-hidden="true" className="rm-setup-check-icon" size={ICON_SIZE.lg} />
        <div><strong>{check.label}<small>{CHECK_LABELS[check.status]}</small></strong><p>{check.detail}</p>{check.advice && check.status !== "ok" ? <p className="rm-setup-advice">{check.advice}</p> : null}</div>
      </li>;
    })}</ul> : <LoadingState label="Checking the server" />}
    <div className="rm-setup-actions">
      <Button busy={isBusy} onClick={() => void run()} variant="secondary"><Refresh size={ICON_SIZE.md} /> Check again</Button>
      <Button disabled={!checks} onClick={onNext} variant="main"><ArrowRight size={ICON_SIZE.md} /> {problems ? "Continue anyway" : "Continue"}</Button>
    </div>
  </>;
}
