// Who joins a workspace without being asked (shared or restricted), and who can open it now and why.
import { useCallback, useEffect, useState } from "react";
import { IconLock as Lock } from "@tabler/icons-react";
import { request } from "../lib/sharedApi";
import { apiMessage } from "../web/format";
import { useConfirm } from "./ui/confirm-dialog";
import { PanelHeading } from "./ui/panel-heading";
import { Switch } from "./ui/switch";
import { Table } from "./ui/table";
import { showToast } from "./ui/toast";

interface Access {
  policy: string;
  members: { user_id: string; email: string | null; display_name: string; role: string; origin: "direct" | "organization" | "group"; via_group: string | null }[];
  system_administrators: { display_name: string; email: string | null }[];
}
interface Preview { become_direct: number; people: string[]; organization_members_not_added?: number }

const ORIGIN = { direct: "Added directly", organization: "Follows the organization role", group: "Granted by a provider group" } as const;

export function WorkspaceAccessPolicy({ accessToken, workspaceId, canChange }: { accessToken: string; workspaceId: string; canChange: boolean }) {
  const confirm = useConfirm();
  const [access, setAccess] = useState<Access | null>(null);
  const [busy, setBusy] = useState(false);
  const load = useCallback(async () => {
    try { setAccess(await request<Access>(`/v1/workspaces/${workspaceId}/effective-access`, {}, accessToken)); }
    catch (error) { showToast("error", apiMessage(error, "Who can open this workspace could not be loaded.")); }
  }, [accessToken, workspaceId]);
  useEffect(() => { void load(); }, [load]);
  if (!access) return null;
  const restricted = access.policy === "restricted";
  async function change(next: boolean) {
    setBusy(true);
    try {
      const preview = await request<Preview>(`/v1/workspaces/${workspaceId}/access-policy/preview`, { method: "POST", body: JSON.stringify({ policy: next ? "restricted" : "shared" }) }, accessToken);
      const effect = next
        ? `Nobody is removed. ${preview.become_direct} ${preview.become_direct === 1 ? "person's access stops" : "people's access stops"} following the organization role and stays as it is.`
        : `Nobody is added. ${preview.organization_members_not_added ?? 0} organization ${preview.organization_members_not_added === 1 ? "member stays" : "members stay"} outside until added one by one; people who join later are added automatically.`;
      if (!(await confirm({ action: next ? "Restrict" : "Share", description: effect, title: next ? "Restrict this workspace?" : "Share this workspace with the organization?" }))) return;
      await request(`/v1/workspaces/${workspaceId}/access-policy`, { method: "PUT", body: JSON.stringify({ policy: next ? "restricted" : "shared" }) }, accessToken);
      await load();
      showToast("success", next ? "The workspace is restricted. Nobody was removed." : "The workspace is shared. Nobody was added.");
    } catch (error) { showToast("error", apiMessage(error, "The access policy could not be changed.")); }
    finally { setBusy(false); }
  }
  return <section aria-label="Workspace access policy" className="shared-settings-group">
    <PanelHeading description={access.members.length} icon={Lock} level={3} title="Who can open this workspace" />
    <p className="shared-muted-copy">{restricted
      ? "Restricted: people who join the organization are not added to this workspace. Add each person below."
      : "Shared: people who join the organization are added to this workspace with a role that follows their organization role."} Changing this adds or removes nobody who is already here.</p>
    {canChange ? <Switch checked={restricted} disabled={busy} label="Restrict this workspace" onCheckedChange={(next) => void change(next)} /> : null}
    <Table>
      <thead><tr><th scope="col">Person</th><th scope="col">Role</th><th scope="col">Why they have access</th></tr></thead>
      <tbody>{access.members.map((member) => <tr key={member.user_id}>
        <th scope="row">{member.display_name}{member.email ? ` · ${member.email}` : ""}</th>
        <td>{member.role}</td>
        <td>{ORIGIN[member.origin]}{member.via_group ? ` (${member.via_group})` : ""}</td>
      </tr>)}</tbody>
    </Table>
    {access.system_administrators.length ? <p className="shared-muted-copy">System administrators of this server can open every workspace without being members: {access.system_administrators.map((admin) => admin.display_name).join(", ")}.</p> : null}
  </section>;
}
