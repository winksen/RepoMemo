import { useMemo, useState } from "react";
import { IconArrowLeft as ArrowLeft, IconArrowRight as ArrowRight } from "@tabler/icons-react";
import { saveSystemSettings } from "../../lib/sharedApi";
import type { SystemSetting } from "../../types";
import { Button } from "../../components/ui/button";
import { Dropdown } from "../../components/ui/dropdown";
import { Input } from "../../components/ui/input";
import { LoadingState } from "../../components/ui/loading-state";
import { Switch } from "../../components/ui/switch";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";
import { SettingValue } from "./steps";
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
        <div><label htmlFor={inputId}><strong>{setting.label}</strong></label><p>{setting.description}</p><small>Server default: {settingValueLabel(setting, setting.default_value)}</small></div>
        <div className="rm-system-setting-control">
          {setting.kind === "boolean"
            ? <Switch checked={Boolean(value)} id={inputId} label={value ? "On" : "Off"} onCheckedChange={(next) => edit(setting, next)} />
            : setting.kind === "choice"
              ? <Dropdown aria-label={setting.label} id={inputId} onValueChange={(next) => edit(setting, next)} options={setting.options} value={String(value)} />
              : <span className="rm-system-number"><Input id={inputId} max={setting.max} min={setting.min} onChange={(event) => edit(setting, event.target.value === "" ? setting.min : Number(event.target.value))} type="number" value={String(value)} /><small>{setting.unit}</small></span>}
        </div>
      </div>;
    })}</div> : <LoadingState label="Loading settings" />}
    <div className="rm-setup-actions">
      <Button disabled={isBusy} onClick={onBack} variant="secondary"><ArrowLeft size={ICON_SIZE.md} /> Back</Button>
      <Button busy={isBusy} disabled={!settings} onClick={() => void save()} variant="main"><ArrowRight size={ICON_SIZE.md} /> {changed.length ? "Save and continue" : "Keep these and continue"}</Button>
    </div>
  </>;
}
