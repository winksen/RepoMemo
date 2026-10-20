import { useMemo, useState } from "react";
import {
  IconBuildingCommunity as Building,
  IconCrown as Crown,
  IconEye as Eye,
  IconKey as Key,
  IconLockOpen as LockOpen,
  IconLogout as Logout,
  IconServerCog as ServerCog,
  IconShieldCheck as ShieldCheck,
  IconShieldOff as ShieldOff,
  IconUser as UserIcon,
  IconUserCheck as UserCheck,
  IconUserCog as UserCog,
  IconUserOff as UserOff,
  IconUsers as Users,
} from "@tabler/icons-react";
import { UserAvatar } from "../../components/UserAvatar";
import {
  anonymizeSystemUser,
  deactivateSystemUser,
  endSystemUserSessions,
  issueSharedPasswordReset,
  listSystemUsers,
  reactivateSystemUser,
  setAppAdmin,
  setSystemAdmin,
  unlockSystemUser,
} from "../../lib/sharedApi";
import type { SharedSession, SystemUser, SystemUserOrganization, SystemUserWorkspace } from "../../types";
import { ActionMenu } from "../../components/ui/action-menu";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Table } from "../../components/ui/table";
import { Dialog, DialogCancel } from "../../components/ui/dialog";
import { EmptyState } from "../../components/ui/empty-state";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { SearchField } from "../../components/ui/search-field";
import { LoadingState } from "../../components/ui/loading-state";
import { Tooltip } from "../../components/ui/tooltip";
import { showToast } from "../../components/ui/toast";
import { appLink, SecretDialog } from "../../components/SecretDialog";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage, formatTime } from "../format";
import { useSystemData, count, RefreshButton } from "./shared";
import { systemKeys } from "../../lib/queryClient";

/** The confirmations that take something away: their button is the danger button. */
const DESTRUCTIVE_KINDS = new Set(["revoke", "revoke-app", "sign-out", "deactivate", "anonymize"]);

const ROLE_ICON = { owner: <Crown size={ICON_SIZE.sm} />, admin: <ShieldCheck size={ICON_SIZE.sm} />, member: <UserIcon size={ICON_SIZE.sm} />, viewer: <Eye size={ICON_SIZE.sm} /> };

function RoleTag({ role, title }: { role: "owner" | "admin" | "member" | "viewer"; title: string }) {
  return <Tooltip content={title}><Badge icon={ROLE_ICON[role]}>{role[0].toUpperCase() + role.slice(1)}</Badge></Tooltip>;
}

/** What a person belongs to, as a tree: each organization on its own line with their role, and under it the workspaces of that organization with their role in each. System administrators are administrators of every organization and workspace; only what they own is listed on top of that. */
function AccessTree({ user }: { user: SystemUser }) {
  // An older server omits the lists; show nothing rather than fail.
  const inherited = user.is_system_admin;
  const groups = new Map<string, { name: string; role: SystemUserOrganization["role"] | null; workspaces: SystemUserWorkspace[] }>();
  for (const organization of user.organizations ?? []) {
    if (inherited && organization.role !== "owner") continue;
    groups.set(organization.organization_id, { name: organization.name, role: organization.role, workspaces: [] });
  }
  for (const workspace of user.workspaces ?? []) {
    if (inherited && workspace.role !== "owner") continue;
    const key = workspace.organization_id ?? "";
    const group = groups.get(key) ?? { name: workspace.organization_name ?? "No organization", role: null, workspaces: [] };
    group.workspaces.push(workspace);
    groups.set(key, group);
  }
  if (!inherited && !groups.size) return <span className="shared-muted-copy">None</span>;
  return <ul className="rm-system-access">
    {inherited ? <li><Tooltip content="System administrators are administrators of every organization and workspace"><Badge icon={<ShieldCheck size={ICON_SIZE.sm} />}>All organizations and workspaces · Admin</Badge></Tooltip></li> : null}
    {Array.from(groups, ([key, group]) => <li key={key}>
      <span className="rm-system-access-org"><Building size={ICON_SIZE.sm} /><strong>{group.name}</strong>{group.role ? <RoleTag role={group.role} title={`${group.role} of the organization ${group.name}`} /> : null}</span>
      {group.workspaces.length ? <ul>{group.workspaces.map((workspace) => <li key={workspace.workspace_id}><span>{workspace.name}</span><RoleTag role={workspace.role} title={`${workspace.role} of the workspace ${workspace.name}`} /></li>)}</ul> : null}
    </li>)}
  </ul>;
}

type UserAction = { kind: "grant" | "revoke" | "grant-app" | "revoke-app" | "sign-out" | "reset" | "deactivate" | "reactivate" | "anonymize"; user: SystemUser };

export function UsersSection({ accessToken, session }: { accessToken: string; session: SharedSession }) {
  const [users, isLoading, reload] = useSystemData<SystemUser[]>(systemKeys.users(), () => listSystemUsers(accessToken));
  const [filter, setFilter] = useState("");
  const [pending, setPending] = useState<UserAction | null>(null);
  const [resetLink, setResetLink] = useState<{ user: SystemUser; url: string; expiresAt: string } | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [confirmText, setConfirmText] = useState("");
  const visible = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    return (users ?? []).filter((user) => !needle || user.display_name.toLowerCase().includes(needle) || user.email.toLowerCase().includes(needle));
  }, [users, filter]);

  async function confirm() {
    if (!pending) return;
    setIsBusy(true);
    try {
      if (pending.kind === "reset") {
        const issued = await issueSharedPasswordReset(accessToken, pending.user.id);
        setResetLink({ user: pending.user, url: appLink(issued.path), expiresAt: issued.expires_at });
        setPending(null);
        return;
      }
      if (pending.kind === "sign-out") {
        await endSystemUserSessions(accessToken, pending.user.id);
        showToast("success", `${pending.user.display_name} was signed out of every device and their API tokens were revoked.`);
      } else if (pending.kind === "deactivate") {
        const ended = await deactivateSystemUser(accessToken, pending.user.id);
        showToast("success", `${pending.user.display_name} is deactivated: signed out everywhere, ${ended.personal_tokens_revoked} API token${ended.personal_tokens_revoked === 1 ? "" : "s"} revoked, ${ended.invitations_withdrawn} invitation${ended.invitations_withdrawn === 1 ? "" : "s"} withdrawn.`);
      } else if (pending.kind === "anonymize") {
        const removed = await anonymizeSystemUser(accessToken, pending.user.id, confirmText);
        showToast("success", `The account is anonymized: ${removed.memberships_removed} membership${removed.memberships_removed === 1 ? "" : "s"} and ${removed.notices_removed} notice${removed.notices_removed === 1 ? "" : "s"} removed. What they wrote now shows as Former member.`);
      } else if (pending.kind === "reactivate") {
        await reactivateSystemUser(accessToken, pending.user.id);
        showToast("success", `${pending.user.display_name} can sign in again with their password.`);
      } else if (pending.kind === "grant-app" || pending.kind === "revoke-app") {
        await setAppAdmin(accessToken, pending.user.id, pending.kind === "grant-app");
        showToast("success", pending.kind === "grant-app" ? `${pending.user.display_name} is now an app administrator.` : `${pending.user.display_name} is no longer an app administrator.`);
      } else {
        await setSystemAdmin(accessToken, pending.user.id, pending.kind === "grant");
        showToast("success", pending.kind === "grant" ? `${pending.user.display_name} is now a system administrator.` : `${pending.user.display_name} is no longer a system administrator.`);
      }
      setPending(null);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The change could not be made.")); }
    finally { setIsBusy(false); }
  }

  async function unlock(user: SystemUser) {
    try {
      const result = await unlockSystemUser(accessToken, user.id);
      showToast("success", result.cleared ? `Sign-in lockouts lifted for ${user.display_name}.` : `${user.display_name} had no sign-in lockout.`);
    } catch (error) { showToast("error", apiMessage(error, "The lockout could not be lifted.")); }
  }

  const dialog = pending ? {
    grant: { title: `Make ${pending.user.display_name} a system administrator?`, description: "They will see every organization and workspace as an administrator, and manage users, settings and maintenance for the whole server.", action: "Make system administrator" },
    revoke: { title: `Remove ${pending.user.display_name} from the system administrators?`, description: pending.user.id === session.user.id ? "You will lose access to the System pages and to workspaces you are not a member of." : "They keep their own organization and workspace memberships.", action: "Remove role" },
    "grant-app": { title: `Make ${pending.user.display_name} an app administrator?`, description: "They can manage users, settings, logs and maintenance for the whole server, but are not an administrator of every organization and workspace: they only reach the ones they belong to.", action: "Make app administrator" },
    "revoke-app": { title: `Remove ${pending.user.display_name} from the app administrators?`, description: pending.user.id === session.user.id ? "You will lose access to the System pages." : "They keep their own organization and workspace memberships.", action: "Remove role" },
    "sign-out": { title: `Sign ${pending.user.display_name} out everywhere?`, description: "Every device signed in to this account is signed out at once, and its personal access tokens are revoked, so scripts using them stop too. Their password is not changed.", action: "Sign out everywhere" },
    deactivate: { title: `Deactivate ${pending.user.display_name}?`, description: "They can no longer sign in or use a token. Their sessions end now, their API tokens are revoked and the invitations they sent are withdrawn. Everything they wrote stays, and so do their memberships. An owner must transfer what they own first. You can reactivate the account later.", action: "Deactivate account" },
    anonymize: { title: `Anonymize ${pending.user.display_name}?`, description: <>Their email address, name, password, picture, memberships, notices and assistant conversations are removed for good, and the address and name are taken out of activity summaries. What they wrote stays, credited to <strong>Former member</strong>. Log files and earlier backups are not rewritten. <strong>This cannot be undone.</strong> Type <code>{pending.user.email}</code> to confirm.</>, action: "Anonymize account" },
    reactivate: { title: `Reactivate ${pending.user.display_name}?`, description: "They can sign in again with the password they had. Sessions, tokens and invitations that ended when the account was deactivated are not brought back.", action: "Reactivate account" },
    reset: { title: `Create a password reset link for ${pending.user.display_name}?`, description: "You get a one-time link to pass on to them. It works for 24 hours; a newer link replaces it. Their current password keeps working until the link is used; then every device of the account is signed out and sign-in lockouts are lifted.", action: "Create link" },
  }[pending.kind] : null;

  return <>
    <div className="rm-system-toolbar">
      <SearchField aria-label="Filter users" onValueChange={setFilter} placeholder="Filter by name or email" value={filter} />
      <RefreshButton isLoading={isLoading} onClick={() => void reload()} />
    </div>
    {users ? <Table>
      <thead><tr><th scope="col">Person</th><th scope="col">Role</th><th scope="col">Organizations and workspaces</th><th scope="col">Sessions</th><th scope="col">Last connected</th><th scope="col">Joined</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
      <tbody>{visible.map((user) => <tr className={user.deactivated_at ? "rm-system-row-inactive" : undefined} key={user.id}>
        <th scope="row"><span className="rm-system-person"><UserAvatar name={user.display_name} size={32} userId={user.id} /><span><strong>{user.display_name}{user.id === session.user.id ? " (you)" : ""}</strong><small>{user.email}</small>{user.deactivated_at ? <small><Tooltip content={`Deactivated ${formatTime(user.deactivated_at)}`}><Badge icon={<UserOff size={ICON_SIZE.sm} />} tone="warning">{user.anonymized_at ? "Anonymized" : "Deactivated"}</Badge></Tooltip></small> : null}</span></span></th>
        <td>{user.is_system_admin ? <Tooltip content="Administers the server and every organization and workspace"><Badge icon={<ServerCog size={ICON_SIZE.sm} />}>Sys Admin</Badge></Tooltip> : user.is_app_admin ? <Tooltip content="Uses the System pages, without access to every organization and workspace"><Badge icon={<UserCog size={ICON_SIZE.sm} />}>App Admin</Badge></Tooltip> : <Badge icon={<UserIcon size={ICON_SIZE.sm} />}>User</Badge>}</td>
        <td><AccessTree user={user} /></td><td>{count(user.active_sessions)}</td><td>{formatTime(user.last_connected_at)}</td><td>{formatTime(user.created_at)}</td>
        <td>{session.is_system_admin && !user.anonymized_at ? <ActionMenu items={[
          user.is_system_admin
            ? { label: "Remove system administrator role", icon: <ShieldOff size={ICON_SIZE.sm} />, onSelect: () => setPending({ kind: "revoke", user }), destructive: true }
            : { label: "Make system administrator", icon: <ShieldCheck size={ICON_SIZE.sm} />, onSelect: () => setPending({ kind: "grant", user }) },
          user.is_app_admin
            ? { label: "Remove app administrator role", icon: <ShieldOff size={ICON_SIZE.sm} />, onSelect: () => setPending({ kind: "revoke-app", user }), destructive: true }
            : { label: "Make app administrator", icon: <UserCog size={ICON_SIZE.sm} />, onSelect: () => setPending({ kind: "grant-app", user }) },
          { label: "Create password reset link", icon: <Key size={ICON_SIZE.sm} />, onSelect: () => setPending({ kind: "reset", user }) },
          { label: "Sign out everywhere", icon: <Logout size={ICON_SIZE.sm} />, onSelect: () => setPending({ kind: "sign-out", user }), destructive: true },
          { label: "Lift sign-in lockout", icon: <LockOpen size={ICON_SIZE.sm} />, onSelect: () => void unlock(user) },
          user.deactivated_at
            ? { label: "Reactivate account", icon: <UserCheck size={ICON_SIZE.sm} />, onSelect: () => setPending({ kind: "reactivate", user }) }
            : { label: "Deactivate account", icon: <UserOff size={ICON_SIZE.sm} />, onSelect: () => setPending({ kind: "deactivate", user }), destructive: true },
          ...(user.deactivated_at && !user.anonymized_at ? [{ label: "Anonymize account", icon: <UserOff size={ICON_SIZE.sm} />, onSelect: () => { setConfirmText(""); setPending({ kind: "anonymize", user }); }, destructive: true }] : []),
        ]} label={`Actions for ${user.display_name}`} /> : null}</td>
      </tr>)}</tbody>
    </Table> : isLoading ? <LoadingState label="Loading users" /> : <EmptyState title="Users are not available" />}
    <Dialog description={dialog?.description} footer={<><DialogCancel onClick={() => setPending(null)} /><Button busy={isBusy} disabled={pending?.kind === "anonymize" && confirmText.trim().toLowerCase() !== pending.user.email.toLowerCase()} onClick={() => void confirm()} variant={pending && DESTRUCTIVE_KINDS.has(pending.kind) ? "danger" : "main"}>{dialog?.action}</Button></>} onClose={() => setPending(null)} open={Boolean(pending)} title={dialog?.title ?? ""}>
      {pending?.kind === "anonymize" ? <Field label="Confirm the email address"><Input autoComplete="off" onChange={(event) => setConfirmText(event.target.value)} placeholder={pending.user.email} value={confirmText} /></Field> : null}
    </Dialog>
    {resetLink ? <SecretDialog description={`Send this link to ${resetLink.user.display_name} (${resetLink.user.email}) through a channel you trust. It lets whoever opens it choose the account's password, once, until ${formatTime(resetLink.expiresAt)}.`} onClose={() => setResetLink(null)} title="Password reset link" value={resetLink.url} /> : null}
  </>;
}
