import { Check } from "@phosphor-icons/react";
import { WORKSPACE_ICONS } from "../web/workspaceIcons";
import { WORKSPACE_COLORS } from "../web/workspaceColors";
import { ICON_SIZE } from "./ui/icon";
import { IconButton } from "./ui/icon-button";
import { Tooltip } from "./ui/tooltip";

/** The icons a workspace can take, as a row of toggle buttons. `value` is the key in use. */
export function WorkspaceIconPicker({ disabled = false, onChange, value }: { disabled?: boolean; onChange: (key: string) => void; value: string }) {
  return (
    <div aria-label="Workspace icon" className="shared-icon-picker" role="group">
      {WORKSPACE_ICONS.map(({ icon: Icon, key, label }) => (
        <IconButton appearance="outline" disabled={disabled} key={key} label={label} onClick={() => onChange(key)} pressed={key === value} size="sm" type="button">
          <Icon size={ICON_SIZE.md} weight={key === value ? "fill" : "regular"} />
        </IconButton>
      ))}
    </div>
  );
}

/** The main colors a workspace can take (and the app's own, `null`), as round swatches. */
export function WorkspaceColorPicker({ disabled = false, onChange, value }: { disabled?: boolean; onChange: (key: string | null) => void; value: string | null }) {
  return (
    <div aria-label="Workspace color" className="shared-color-picker" role="group">
      {[{ key: null, label: "Default", light: null, dark: null }, ...WORKSPACE_COLORS].map((color) => {
        const selected = color.key === value;
        return <Tooltip content={color.label} key={color.key ?? "default"}>
          <button aria-label={color.label} aria-pressed={selected} className="shared-color-swatch" data-default={color.key === null ? "true" : undefined} disabled={disabled} onClick={() => onChange(color.key)} style={color.light && color.dark ? { "--swatch-light": color.light.accent, "--swatch-dark": color.dark.accent, "--swatch-on-light": color.light.on, "--swatch-on-dark": color.dark.on } as React.CSSProperties : undefined} type="button">
            {selected ? <Check size={ICON_SIZE.sm} /> : null}
          </button>
        </Tooltip>;
      })}
    </div>
  );
}
