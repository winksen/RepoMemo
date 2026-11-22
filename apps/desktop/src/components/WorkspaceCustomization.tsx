import { useState } from "react";
import { Palette as PaletteIcon } from "@phosphor-icons/react";
import { saveSharedWorkspaceColor, saveSharedWorkspaceIcon } from "../lib/sharedApi";
import { workspaceIconKey } from "../web/workspaceIcons";
import { workspaceColorKey } from "../web/workspaceColors";
import { WorkspaceColorPicker, WorkspaceIconPicker } from "./WorkspaceLookPickers";
import type { Workspace } from "../types";
import { apiMessage } from "../web/format";
import { PanelHeading } from "./ui/panel-heading";
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
      <div className="shared-new-workspace-look">
        <div><strong>Icon</strong><WorkspaceIconPicker disabled={pending !== null} onChange={(key) => void chooseIcon(key)} value={currentIcon} /></div>
        <div><strong>Main color</strong><WorkspaceColorPicker disabled={pending !== null} onChange={(key) => void chooseColor(key)} value={currentColor} /></div>
      </div>
    </section>
  );
}
