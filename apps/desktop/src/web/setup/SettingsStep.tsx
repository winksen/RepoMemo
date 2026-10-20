import { useMemo, useState } from "react";
import { IconArrowLeft as ArrowLeft, IconArrowRight as ArrowRight } from "@tabler/icons-react";
import { saveSystemSettings } from "../../lib/sharedApi";
import type { SystemSetting } from "../../types";
import { Button } from "../../components/ui/button";
import { LoadingState } from "../../components/ui/loading-state";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";
import { SettingValue } from "./steps";
import { SettingControl, settingProblem } from "../system/SettingControl";
import { StepHeading } from "./StepFrame";

export function settingValueLabel(setting: SystemSetting, value: SettingValue): string {
  if (setting.kind === "boolean") return value ? "On" : "Off";
  if (setting.kind === "choice") return setting.options.find((option) => option.value === value)?.label ?? String(value);
  return `${value}${setting.unit ? ` ${setting.unit}` : ""}`;
}

export function SettingsStep({ accessToken, description, keys, onBack, onNext, onSaved, settings, title }: {
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
  const hasProblem = shown.some((setting) => setting.key in draft && settingProblem(setting, draft[setting.key]) !== null);

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
    } catch (error) { showToast("error", apiMessage(error, "The settings could not be saved.")); }
    finally { setIsBusy(false); }
  }

  const position = title === "Logging" ? 5 : 4;
  return <>
    <StepHeading eyebrow={`Step ${position} of 7`} title={title}>{description}</StepHeading>
    {settings ? <div className="rm-system-settings">{shown.map((setting) => {
      const value = draft[setting.key] ?? setting.value;
      const inputId = `setup-setting-${setting.key}`;
      return <div className="rm-system-setting" key={setting.key}>
        <div><label htmlFor={inputId}><strong>{setting.label}</strong></label><p id={`${inputId}-description`}>{setting.description}</p><small>Server default: {settingValueLabel(setting, setting.default_value)}</small></div>
        <div className="rm-system-setting-control">
          <SettingControl describedBy={`${inputId}-description`} id={inputId} onChange={(next) => edit(setting, next)} setting={setting} value={value} />
        </div>
      </div>;
    })}</div> : <LoadingState label="Loading settings" />}
    <div className="rm-setup-actions">
      <Button disabled={isBusy} onClick={onBack} variant="secondary"><ArrowLeft size={ICON_SIZE.md} /> Back</Button>
      <Button busy={isBusy} disabled={!settings || hasProblem} onClick={() => void save()} variant="main"><ArrowRight size={ICON_SIZE.md} /> {changed.length ? "Save and continue" : "Keep these and continue"}</Button>
    </div>
  </>;
}
