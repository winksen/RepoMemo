import { useState } from "react";
import { IconCheck as Check, IconPalette as PaletteIcon } from "@tabler/icons-react";
import { saveSharedWorkspaceColor, saveSharedWorkspaceIcon } from "../lib/sharedApi";
import { WORKSPACE_ICONS, workspaceIconKey } from "../web/workspaceIcons";
import { WORKSPACE_COLORS, workspaceColorKey } from "../web/workspaceColors";
import type { Workspace } from "../types";
import { apiMessage } from "../web/format";
import { ICON_SIZE } from "./ui/icon";
import { IconButton } from "./ui/icon-button";
import { PanelHeading } from "./ui/panel-heading";
import { Tooltip } from "./ui/tooltip";
import { showToast } from "./ui/toast";

/**
 * How a workspace looks: the icon it shows in lists and menus, and the main colour of its tabs and highlights.
 * Shown to owners and administrators; each choice is saved at once. Everyone who opens the workspace sees both.
 */
export function WorkspaceCustomization({ accessToken, onSaved, workspace }: {
  accessToken: string;
  /** Called with the workspace as it is now, so lists and the open page show the change. */
  onSaved: (workspace: Workspace) => void;
  workspace: Workspace;
}) {
  const [pending, setPending] = useState<string | null>(null);
  const currentIcon = workspaceIconKey(workspace);
  const currentColor = workspaceColorKey(workspace);

  async function chooseIcon(key: string) {
    if (key === currentIcon || pending) return;
    setPending(`icon:${key}`);
    try {
      const saved = await saveSharedWorkspaceIcon(accessToken, workspace.id, key);
      onSaved({ ...workspace, settings: { ...workspace.settings, icon: saved.icon } });
      showToast("success", "Workspace icon updated.");
    } catch (requestError) {
      showToast("error", apiMessage(requestError, "The workspace icon could not be saved."));
    } finally { setPending(null); }
  }

  async function chooseColor(key: string | null) {
    if (key === currentColor || pending) return;
    setPending(`color:${key ?? "default"}`);
    try {
      const saved = await saveSharedWorkspaceColor(accessToken, workspace.id, key);
      onSaved({ ...workspace, settings: { ...workspace.settings, color: saved.color } });
      showToast("success", "Workspace color updated.");
    } catch (requestError) {
      showToast("error", apiMessage(requestError, "The workspace color could not be saved."));
    } finally { setPending(null); }
  }

  return (
    <section className="shared-settings-group">
      <PanelHeading icon={PaletteIcon} title="Customization" />
      <p className="shared-muted-copy">Choose how this workspace looks. Everyone in the organization sees the icon and the color.</p>
      <div className="shared-customization-block">
        <strong>Icon</strong>
        <span className="shared-muted-copy">Stands for the workspace in the dashboard, the workspace lists and the sidebar.</span>
        <div aria-label="Workspace icon" className="shared-icon-picker" role="group">
          {WORKSPACE_ICONS.map(({ icon: Icon, key, label }) => (
            <IconButton appearance="outline" disabled={pending !== null && pending !== `icon:${key}`} key={key} label={label} onClick={() => void chooseIcon(key)} pressed={key === currentIcon}>
              <Icon size={ICON_SIZE.lg} />
            </IconButton>
          ))}
        </div>
      </div>
      <div className="shared-customization-block">
        <strong>Main color</strong>
        <span className="shared-muted-copy">Colors the tabs and highlights of this workspace. Each color has a darker shade for the light theme and a lighter one for the dark theme, so it reads well on both.</span>
        <div aria-label="Workspace color" className="shared-color-picker" role="group">
          {[{ key: null, label: "Default", light: null, dark: null }, ...WORKSPACE_COLORS].map((color) => {
            const selected = color.key === currentColor;
            return <Tooltip content={color.label} key={color.key ?? "default"}>
              <button aria-label={color.label} aria-pressed={selected} className="shared-color-swatch" data-default={color.key === null ? "true" : undefined} disabled={pending !== null && pending !== `color:${color.key ?? "default"}`} onClick={() => void chooseColor(color.key)} style={color.light && color.dark ? { "--swatch-light": color.light.accent, "--swatch-dark": color.dark.accent, "--swatch-on-light": color.light.on, "--swatch-on-dark": color.dark.on } as React.CSSProperties : undefined} type="button">
                {selected ? <Check size={ICON_SIZE.md} /> : null}
              </button>
            </Tooltip>;
          })}
        </div>
      </div>
    </section>
  );
}
