import { useState } from "react";
import { IconPhoto as Photo } from "@tabler/icons-react";
import { saveSharedWorkspaceIcon } from "../lib/sharedApi";
import { WORKSPACE_ICONS, workspaceIconKey } from "../web/workspaceIcons";
import type { Workspace } from "../types";
import { apiMessage } from "../web/format";
import { ICON_SIZE } from "./ui/icon";
import { IconButton } from "./ui/icon-button";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";

/** Which icon a workspace shows in lists and menus. Shown to owners and administrators; a choice is saved at once. */
export function WorkspaceIconSettings({ accessToken, onSaved, workspace }: {
  accessToken: string;
  /** Called with the workspace as it is now, so lists show the new icon. */
  onSaved: (workspace: Workspace) => void;
  workspace: Workspace;
}) {
  const [pendingKey, setPendingKey] = useState<string | null>(null);
  const current = workspaceIconKey(workspace);

  async function choose(key: string) {
    if (key === current || pendingKey) return;
    setPendingKey(key);
    try {
      const saved = await saveSharedWorkspaceIcon(accessToken, workspace.id, key);
      onSaved({ ...workspace, settings: { ...workspace.settings, icon: saved.icon } });
      showToast("success", "Workspace icon updated.");
    } catch (requestError) {
      showToast("error", apiMessage(requestError, "The workspace icon could not be saved."));
    } finally { setPendingKey(null); }
  }

  return (
    <section className="shared-settings-group">
      <PanelHeading icon={Photo} title="Workspace icon" />
      <p className="shared-muted-copy">Pick the icon that stands for this workspace in the dashboard, the workspace lists and the organization menu. Everyone in the organization sees it.</p>
      <div aria-label="Workspace icon" className="shared-icon-picker" role="group">
        {WORKSPACE_ICONS.map(({ icon: Icon, key, label }) => (
          <IconButton appearance="outline" disabled={pendingKey !== null && pendingKey !== key} key={key} label={label} onClick={() => void choose(key)} pressed={key === current}>
            <Icon size={ICON_SIZE.lg} />
          </IconButton>
        ))}
      </div>
    </section>
  );
}
