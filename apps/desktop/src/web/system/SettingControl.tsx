import type { SystemSetting } from "../../types";
import { Dropdown } from "../../components/ui/dropdown";
import { Input } from "../../components/ui/input";
import { Switch } from "../../components/ui/switch";

export type SettingValue = boolean | number | string;

/** What is wrong with a whole number outside the range the server accepts; other kinds cannot be wrong. */
export function settingProblem(setting: SystemSetting, value: SettingValue): string | null {
  if (setting.kind !== "integer") return null;
  const number = Number(value);
  if (Number.isInteger(number) && number >= setting.min && number <= setting.max) return null;
  return `Between ${setting.min.toLocaleString()} and ${setting.max.toLocaleString()}${setting.unit ? ` ${setting.unit}` : ""}.`;
}

/**
 * The control of one setting row: a switch for on and off, a dropdown for a choice and a number field
 * for a whole number. A number outside its range is marked invalid with a one-line message under it.
 * The row's label is tied to it with `id`; the row's description with `describedBy`.
 */
export function SettingControl({ describedBy, disabled = false, id, onChange, setting, value }: {
  describedBy?: string;
  disabled?: boolean;
  id: string;
  onChange: (value: SettingValue) => void;
  setting: SystemSetting;
  value: SettingValue;
}) {
  if (setting.kind === "boolean") {
    return <Switch checked={Boolean(value)} disabled={disabled} id={id} label={value ? "On" : "Off"} onCheckedChange={onChange} />;
  }
  if (setting.kind === "choice") {
    return <Dropdown aria-label={setting.label} disabled={disabled} id={id} onValueChange={onChange} options={setting.options} value={String(value)} />;
  }
  const problem = settingProblem(setting, value);
  const messageId = problem ? `${id}-message` : undefined;
  return <>
    <span className="rm-system-number">
      <Input
        aria-describedby={[describedBy, messageId].filter(Boolean).join(" ") || undefined}
        aria-invalid={problem ? true : undefined}
        disabled={disabled}
        id={id}
        max={setting.max}
        min={setting.min}
        onChange={(event) => onChange(event.target.value === "" ? setting.min : Number(event.target.value))}
        type="number"
        value={String(value)}
      />
      <small>{setting.unit}</small>
    </span>
    {problem ? <small className="rm-field-message rm-setting-problem" id={messageId}>{problem}</small> : null}
  </>;
}
