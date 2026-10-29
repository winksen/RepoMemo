// Who joins a workspace without being asked: everyone in the organization (shared) or only the people added one by one.
import { useEffect, useState } from "react";
import { IconLock as Lock } from "@tabler/icons-react";
import { request } from "../lib/sharedApi";
import { apiMessage } from "../web/format";
import { PanelHeading } from "./ui/panel-heading";
import { Switch } from "./ui/switch";
import { showToast } from "./ui/toast";

export function WorkspaceAccessPolicy({ accessToken, workspaceId, canChange }: { accessToken: string; workspaceId: string; canChange: boolean }) {
  const [restricted, setRestricted] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let current = true;
    request<{ policy: string }>(`/v1/workspaces/${workspaceId}/access-policy`, {}, accessToken)
      .then((answer) => { if (current) setRestricted(answer.policy === "restricted"); })
      .catch((error) => showToast("error", apiMessage(error, "The access policy could not be loaded.")));
    return () => { current = false; };
  }, [accessToken, workspaceId]);
  if (restricted === null) return null;
  async function change(next: boolean) {
    setBusy(true);
    try {
      await request(`/v1/workspaces/${workspaceId}/access-policy`, { method: "PUT", body: JSON.stringify({ policy: next ? "restricted" : "shared" }) }, accessToken);
      setRestricted(next);
      showToast("success", next ? "The workspace is restricted. Nobody was removed." : "The workspace is shared. Nobody was added.");
    } catch (error) { showToast("error", apiMessage(error, "The access policy could not be changed.")); }
    finally { setBusy(false); }
  }
  return <section aria-label="Workspace access policy" className="shared-settings-group">
    <PanelHeading icon={Lock} level={3} title="Who joins automatically" />
    <p className="shared-muted-copy">{restricted
      ? "Restricted: people who join the organization are not added to this workspace. Add each person below."
      : "Shared: people who join the organization are added to this workspace with a role that follows their organization role."} Changing this adds or removes nobody who is already here.</p>
    {canChange ? <Switch checked={restricted} disabled={busy} label="Restrict this workspace" onCheckedChange={(next) => void change(next)} /> : null}
  </section>;
}
