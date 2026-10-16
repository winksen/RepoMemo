import { useEffect, useMemo, useRef, useState } from "react";
import { MailSettingsPanel } from "./MailSettingsPanel";
import type { Dispatch, SetStateAction } from "react";
import type { ReactNode } from "react";
import {
  IconActivity as Activity,
  IconAdjustments as Adjustments,
  IconAlertCircle as AlertCircle,
  IconCircleCheck as CircleCheck,
  IconChartBar as ChartBar,
  IconClipboardList as ClipboardList,
  IconBuildingCommunity as Building,
  IconCrown as Crown,
  IconDatabase as Database,
  IconDownload as Download,
  IconEye as Eye,
  IconFileText as FileText,
  IconKey as Key,
  IconLoader2 as Loader,
  IconLockOpen as LockOpen,
  IconLogout as Logout,
  IconRefresh as Refresh,
  IconRotate as Rotate,
  IconServer as Server,
  IconPlugConnectedX as Unplug,
  IconServerCog as ServerCog,
  IconShieldCheck as ShieldCheck,
  IconShieldOff as ShieldOff,
  IconTerminal2 as Terminal,
  IconTool as Tool,
  IconUser as UserIcon,
  IconUserCheck as UserCheck,
  IconUserCog as UserCog,
  IconUserOff as UserOff,
  IconUsers as Users,
  IconWorld as World,
} from "@tabler/icons-react";
import { SystemConsole } from "./SystemConsole";
import { UserAvatar } from "./UserAvatar";
import {
  cancelSharedJob,
  anonymizeSystemUser,
  copySystemBackups,
  createSystemBackup,
  deactivateSystemUser,
  deleteSystemBackup,
  detachEnvironment,
  downloadSystemLogFile,
  endSystemUserSessions,
  issueSharedPasswordReset,
  getPlatformStatus,
  getSystemBackups,
  getSystemLogFiles,
  getSystemLogs,
  getSystemOverview,
  getSystemSettings,
  getSystemUsage,
  clearSharedSession,
  listSystemAuditEvents,
  listSystemJobs,
  listSystemUsers,
  reactivateSystemUser,
  resetSystemSetting,
  restoreSystemBackup,
  runSystemMaintenance,
  saveSystemSettings,
  setAppAdmin,
  setSystemAdmin,
  unlockSystemUser,
} from "../lib/sharedApi";
import type {
  BackupsOverview,
  CountByLabel,
  MaintenanceStatus,
  SharedSession,
  SharedWorkspace,
  SystemAuditEvent,
  SystemJob,
  SystemLogFiles,
  SystemLogs,
  SystemOverview,
  SystemSetting,
  SystemSettings,
  SystemUsage,
  SystemUser,
  SystemUserOrganization,
  SystemUserWorkspace,
} from "../types";
import { ActionMenu } from "./ui/action-menu";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Dropdown } from "./ui/dropdown";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";
import { useLiveEventHub, useLiveEvents } from "../lib/liveEvents";
import { appLink, SecretDialog } from "./SecretDialog";

import { SYSTEM_SECTIONS, type SystemSection } from "../web/constants";
export { SYSTEM_SECTIONS };
export type { SystemSection };

const SECTION_LABELS: Record<SystemSection, { label: string; icon: ReactNode }> = {
  overview: { label: "Overview", icon: <Server size={16} /> },
  usage: { label: "Usage", icon: <ChartBar size={16} /> },
  users: { label: "Users", icon: <Users size={16} /> },
  settings: { label: "Settings", icon: <Adjustments size={16} /> },
  logs: { label: "Logs", icon: <FileText size={16} /> },
  jobs: { label: "Jobs & maintenance", icon: <Tool size={16} /> },
  audit: { label: "Audit trail", icon: <ClipboardList size={16} /> },
  console: { label: "Console", icon: <Terminal size={16} /> },
};

/** Tabs of the System area, shown in the layout's navigation slot. */
export function SystemNavigation({ active, onNavigate }: { active: SystemSection; onNavigate: (section: SystemSection) => void }) {
  return <nav aria-label="System sections" className="shared-workspace-topbar">
    <div className="shared-workspace-tabs">
      {SYSTEM_SECTIONS.map((section) => {
        const isActive = section === active;
        return <Button aria-current={isActive ? "page" : undefined} className={isActive ? "active" : ""} key={section} onClick={() => onNavigate(section)} type="button" variant="secondary">{SECTION_LABELS[section].icon}<span className="shared-workspace-tab-label">{SECTION_LABELS[section].label}</span></Button>;
      })}
    </div>
  </nav>;
}

const SECTION_INTRO: Record<SystemSection, string> = {
  overview: "This server instance, the clients calling it, its storage and its background work.",
  usage: "How the organizations and workspaces on this server are used.",
  users: "Every account on the server. System administrators act as administrators in every organization and workspace.",
  settings: "Settings that apply to the whole server. Changes take effect at once and are recorded in the audit trail.",
  logs: "Server log events: recent ones kept in memory, and earlier days from the log files. What is recorded is set under Settings › Logging.",
  jobs: "Background jobs across every workspace, the maintenance that keeps the server tidy, and backups.",
  audit: "What system administrators did, kept durably.",
  console: "A command line to this server for technical administrators: the same actions as these pages, typed.",
};

/** The System area for system administrators. */
export function SystemAdminPanel({ accessToken, onNavigateSection, onOpenWorkspace, section, session, workspaces }: {
  accessToken: string;
  onNavigateSection: (section: SystemSection) => void;
  onOpenWorkspace: (workspaceId: string) => void;
  section: SystemSection;
  session: SharedSession;
  /** Every workspace (system administrators see all), for the log filter. */
  workspaces: SharedWorkspace[];
}) {
  return <section className="shared-page-content rm-system-page">
    <div className="shared-page-heading"><div><h1>System · {SECTION_LABELS[section].label}</h1><p>{SECTION_INTRO[section]}</p></div></div>
    {section === "overview" ? <OverviewSection accessToken={accessToken} /> : null}
    {section === "usage" ? <UsageSection accessToken={accessToken} onOpenWorkspace={onOpenWorkspace} /> : null}
    {section === "users" ? <UsersSection accessToken={accessToken} session={session} /> : null}
    {section === "settings" ? <SettingsSection accessToken={accessToken} session={session} /> : null}
    {section === "logs" ? <LogsSection accessToken={accessToken} canDownload={session.is_system_admin === true} onOpenSettings={() => onNavigateSection("settings")} workspaces={workspaces} /> : null}
    {section === "jobs" ? <JobsSection accessToken={accessToken} canChange={session.is_system_admin === true} /> : null}
    {section === "audit" ? <AuditSection accessToken={accessToken} /> : null}
    {section === "console" ? <SystemConsole accessToken={accessToken} /> : null}
  </section>;
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

/** Loads data, reloads on demand and, optionally, every `refreshMs`. */
function useSystemData<T>(load: () => Promise<T>, dependencies: unknown[], refreshMs?: number): [T | null, boolean, () => Promise<void>, Dispatch<SetStateAction<T | null>>] {
  const [data, setData] = useState<T | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  async function reload() {
    setIsLoading(true);
    try { setData(await load()); }
    catch (error) { showToast("error", errorMessage(error, "The system information could not be loaded.")); }
    finally { setIsLoading(false); }
  }
  useEffect(() => {
    void reload();
    if (!refreshMs) return undefined;
    const timer = window.setInterval(() => { void load().then(setData).catch(() => undefined); }, refreshMs);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, dependencies);
  return [data, isLoading, reload, setData];
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

function formatDuration(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days) return `${days} d ${hours} h`;
  if (hours) return `${hours} h ${minutes} min`;
  return `${minutes} min`;
}

function formatTime(value: string | null | undefined): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function count(value: number): string {
  return value.toLocaleString();
}

function RefreshButton({ isLoading, onClick }: { isLoading: boolean; onClick: () => void }) {
  return <Button disabled={isLoading} onClick={onClick} type="button" variant="secondary">{isLoading ? <Loader className="spin" size={16} /> : <Refresh size={16} />} Refresh</Button>;
}

function Facts({ items }: { items: [string, ReactNode][] }) {
  return <dl className="rm-system-facts">{items.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>;
}

/** A single-series bar strip: one bar per value, newest last, with a hover label per bar. */
function BarStrip({ label, values }: { label: string; values: { label: string; value: number }[] }) {
  const max = Math.max(1, ...values.map((entry) => entry.value));
  return <div aria-label={label} className="rm-system-bars" role="img">
    {values.map((entry) => <span className="rm-system-bar" key={entry.label} title={`${entry.label}: ${entry.value.toLocaleString()}`}>
      <i style={{ height: `${entry.value ? Math.max(4, (entry.value / max) * 100) : 0}%` }} />
    </span>)}
  </div>;
}

function CountTable({ rows, label, valueLabel }: { rows: CountByLabel[]; label: string; valueLabel: string }) {
  return rows.length ? <div className="rm-map-coverage"><table>
    <thead><tr><th scope="col">{label}</th><th scope="col">{valueLabel}</th></tr></thead>
    <tbody>{rows.map((row) => <tr key={row.label}><th scope="row">{row.label.replace(/_/g, " ")}</th><td>{count(row.count)}</td></tr>)}</tbody>
  </table></div> : <p className="shared-muted-copy">Nothing recorded in this period.</p>;
}

function MaintenancePanel({ accessToken, status, onChanged }: { accessToken: string; status: MaintenanceStatus | null; onChanged: (status: MaintenanceStatus) => void }) {
  const [isRunning, setIsRunning] = useState(false);
  async function run() {
    setIsRunning(true);
    try {
      const next = await runSystemMaintenance(accessToken);
      onChanged(next);
      const report = next.last_report;
      showToast(report?.failures ? "warning" : "success", report
        ? `Maintenance finished: ${report.blobs_removed} stored files removed (${formatBytes(report.blob_bytes_reclaimed)}), ${report.jobs_pruned} old jobs and ${report.refresh_tokens_purged} expired sessions cleared${report.failures ? `, ${report.failures} step(s) failed — see Logs` : ""}.`
        : "Maintenance finished.");
    } catch (error) { showToast("error", errorMessage(error, "Maintenance could not run.")); }
    finally { setIsRunning(false); }
  }
  const report = status?.last_report;
  return <section className="shared-settings-group">
    <div className="shared-panel-heading"><div><Tool size={18} /><h2>Maintenance</h2></div><span>{status?.running ? "running" : status?.next_run_at ? `next ${formatTime(status.next_run_at)}` : "scheduled runs off"}</span></div>
    <p className="shared-muted-copy">Clears expired sessions and old jobs, deletes stored files nothing references, retries failed indexing and embeddings, and checkpoints the database.</p>
    <Facts items={[
      ["Last run", status?.last_finished_at ? `${formatTime(status.last_finished_at)} (${status.last_trigger ?? "scheduled"})` : "not yet since the server started"],
      ["Stored files removed", report ? `${count(report.blobs_removed)} · ${formatBytes(report.blob_bytes_reclaimed)}` : "—"],
      ["Cached previews removed", report ? count(report.previews_removed) : "—"],
      ["Old jobs removed", report ? count(report.jobs_pruned) : "—"],
      ["Expired sessions removed", report ? count(report.refresh_tokens_purged) : "—"],
      ["Failed steps", report ? count(report.failures) : "—"],
    ]} />
    <div><Button disabled={isRunning || status?.running} onClick={() => void run()} type="button" variant="main">{isRunning ? <Loader className="spin" size={16} /> : <Rotate size={16} />} Run maintenance now</Button></div>
  </section>;
}

function OverviewSection({ accessToken }: { accessToken: string }) {
  const [overview, isLoading, reload] = useSystemData<SystemOverview>(() => getSystemOverview(accessToken), [accessToken], 30_000);
  const [maintenance, setMaintenance] = useState<MaintenanceStatus | null>(null);
  useEffect(() => { if (overview) setMaintenance(overview.background.maintenance); }, [overview]);
  if (!overview) return <p className="shared-muted-copy">{isLoading ? <><Loader className="spin" size={14} /> Loading the system overview…</> : "The overview is not available."}</p>;
  const { instance, statistics, storage, background, traffic, protection } = overview;
  const minuteBars = traffic.requests_per_minute_last_hour.map((value, index, all) => ({ label: `${all.length - 1 - index} min ago`, value }));
  return <>
    <div className="rm-system-toolbar"><span className="shared-muted-copy">Refreshes every 30 seconds.</span><RefreshButton isLoading={isLoading} onClick={() => void reload()} /></div>
    <dl className="shared-dashboard-summary">
      <div><dt>Users</dt><dd>{count(statistics.user_count)}</dd><span>{count(statistics.users_active_last_7_days)} active this week</span></div>
      <div><dt>Organizations</dt><dd>{count(statistics.organization_count)}</dd><span>{count(statistics.workspace_count)} workspaces</span></div>
      <div><dt>Evidence</dt><dd>{count(statistics.artifact_count)}</dd><span>{formatBytes(statistics.artifact_bytes)} · {count(statistics.indexed_artifact_count)} indexed</span></div>
      <div><dt>Sessions</dt><dd>{count(statistics.active_session_count)}</dd><span>signed in, any device</span></div>
      <div><dt>Requests</dt><dd>{count(traffic.requests_last_hour)}</dd><span>in the last hour</span></div>
    </dl>
    <div className="rm-system-grid">
      <section className="shared-settings-group">
        <div className="shared-panel-heading"><div><Server size={18} /><h2>Back end</h2></div><span>up {formatDuration(instance.uptime_seconds)}</span></div>
        <Facts items={[
          ["Service", `${instance.service_name} ${instance.version}`],
          ["Host", `${instance.host_name} · ${instance.operating_system}/${instance.architecture}`],
          ["Process", String(instance.process_id)],
          ["Listening on", instance.bind_address],
          ["Data folder", <code key="dir">{instance.data_dir}</code>],
          ["Started", formatTime(instance.started_at)],
          ["System administrators", count(instance.system_admin_count)],
          ["Log capture", overview.logs.capturing ? `${count(overview.logs.buffered)} events held` : "off"],
        ]} />
      </section>
      <section className="shared-settings-group">
        <div className="shared-panel-heading"><div><Database size={18} /><h2>Storage</h2></div><span>{formatBytes(storage.database_bytes + storage.write_ahead_log_bytes + storage.blob_bytes + storage.preview_bytes)}</span></div>
        <Facts items={[
          ["Database", formatBytes(storage.database_bytes)],
          ["Write-ahead log", formatBytes(storage.write_ahead_log_bytes)],
          ["Stored files", `${count(statistics.blob_count)} · ${formatBytes(storage.blob_bytes)}`],
          ["Cached previews", `${count(storage.preview_count)} · ${formatBytes(storage.preview_bytes)}`],
          ["Free disk", storage.disk_free_bytes == null ? "unknown" : `${formatBytes(storage.disk_free_bytes)}${storage.disk_floor_bytes ? ` · uploads stop below ${formatBytes(storage.disk_floor_bytes)}` : ""}`],
          ["Storage per workspace", storage.workspace_storage_bytes ? formatBytes(storage.workspace_storage_bytes) : "unlimited"],
          ["Remote repository copies", `${count(storage.remote_clones.length)} · ${formatBytes(storage.remote_clone_bytes)}${storage.remote_clone_limit_bytes ? ` · each stops at ${formatBytes(storage.remote_clone_limit_bytes)}` : ""}`],
          ...storage.remote_clones.slice(0, 5).map((clone): [string, string] => [`  ${clone.name}`, formatBytes(clone.bytes)]),
          ["Passages", count(statistics.chunk_count)],
          ["Search vectors", count(statistics.embedding_count)],
          ["Memory cards · tasks · comments", `${count(statistics.memory_card_count)} · ${count(statistics.task_count)} · ${count(statistics.comment_count)}`],
          ["Repositories", count(statistics.repository_count)],
        ]} />
      </section>
      <section className="shared-settings-group">
        <div className="shared-panel-heading"><div><Activity size={18} /><h2>Background work</h2></div><span>{background.indexing_queued + background.repository_syncs_running ? "busy" : "idle"}</span></div>
        <Facts items={[
          ["Files waiting for indexing", count(background.indexing_queued)],
          ["Workspaces waiting for vectors", count(background.embedding_workspaces_waiting)],
          ["Repository syncs", count(background.repository_syncs_running)],
          ["Preview conversions", count(background.preview_conversions_running)],
          ["LibreOffice", background.office_previews.available ? `${background.office_previews.version ?? "version not read yet"} · ${background.office_previews.sandboxed ? "in a sandbox" : "no sandbox"}` : "not installed"],
          ["Jobs", statistics.jobs_by_status.map((entry) => `${count(entry.count)} ${entry.label}`).join(" · ") || "none"],
          ["Live event streams", `${count(background.open_event_streams)} open · ${count(background.event_channels)} workspaces watched`],
          ["AI providers", `${count(statistics.enabled_provider_count)} enabled · ${count(statistics.cloud_provider_count)} cloud`],
        ]} />
      </section>
      <section className="shared-settings-group">
        <div className="shared-panel-heading"><div><ShieldCheck size={18} /><h2>Traffic and protection</h2></div><span>since start</span></div>
        <Facts items={[
          ["Requests", count(traffic.requests)],
          ["Client errors · server errors", `${count(traffic.client_errors)} · ${count(traffic.server_errors)}`],
          ["Refused sign-ins and tokens (401)", count(traffic.unauthorized)],
          ["Refused access (403)", count(traffic.forbidden)],
          ["Rate limited (429)", count(traffic.rate_limited)],
          ["Locked sign-ins", count(protection.locked_sign_in_keys)],
        ]} />
        <div className="rm-system-chart"><p className="rm-system-chart-title">Requests per minute, last hour</p><BarStrip label="Requests per minute over the last hour" values={minuteBars} /></div>
      </section>
    </div>
    <section className="shared-settings-group">
      <div className="shared-panel-heading"><div><World size={18} /><h2>Front-end clients</h2></div><span>seen in the last 24 hours</span></div>
      {overview.clients.length ? <div className="rm-map-coverage"><table>
        <thead><tr><th scope="col">Client</th><th scope="col">Origin</th><th scope="col">Last address</th><th scope="col">Requests</th><th scope="col">First seen</th><th scope="col">Last seen</th></tr></thead>
        <tbody>{overview.clients.map((client) => <tr key={`${client.origin}-${client.client}-${client.agent}`}>
          <th scope="row">{client.agent}{client.client ? <small className="rm-system-tag">{client.client}</small> : null}</th>
          <td>{client.origin ?? "no origin (tool or direct link)"}</td><td><code>{client.last_address}</code></td><td>{count(client.requests)}</td><td>{formatTime(client.first_seen_at)}</td><td>{formatTime(client.last_seen_at)}</td>
        </tr>)}</tbody>
      </table></div> : <p className="shared-muted-copy">No clients seen yet.</p>}
    </section>
    <MaintenancePanel accessToken={accessToken} onChanged={setMaintenance} status={maintenance} />
  </>;
}

function UsageSection({ accessToken, onOpenWorkspace }: { accessToken: string; onOpenWorkspace: (workspaceId: string) => void }) {
  const [days, setDays] = useState(30);
  const [usage, isLoading, reload] = useSystemData<SystemUsage>(() => getSystemUsage(accessToken, days), [accessToken, days]);
  const total = usage?.activity_by_day.reduce((sum, entry) => sum + entry.count, 0) ?? 0;
  return <>
    <div className="rm-system-toolbar">
      <label className="rm-system-inline">Period<Dropdown aria-label="Usage period" onValueChange={(value) => setDays(Number(value))} options={[{ label: "Last 7 days", value: "7" }, { label: "Last 30 days", value: "30" }, { label: "Last 90 days", value: "90" }, { label: "Last 365 days", value: "365" }]} value={String(days)} /></label>
      <RefreshButton isLoading={isLoading} onClick={() => void reload()} />
    </div>
    {usage ? <>
      <section className="shared-settings-group">
        <div className="shared-panel-heading"><div><ChartBar size={18} /><h2>Recorded activity per day</h2></div><span>{count(total)} actions</span></div>
        <BarStrip label={`Recorded activity per day over the last ${usage.days} days`} values={usage.activity_by_day.map((entry) => ({ label: entry.label, value: entry.count }))} />
        <div className="rm-system-axis"><span>{usage.activity_by_day[0]?.label}</span><span>{usage.activity_by_day[usage.activity_by_day.length - 1]?.label}</span></div>
      </section>
      <div className="rm-system-grid">
        <section className="shared-settings-group"><div className="shared-panel-heading"><div><Activity size={18} /><h2>Activity by kind</h2></div></div><CountTable label="Action" rows={usage.activity_by_action} valueLabel="Times" /></section>
        <section className="shared-settings-group"><div className="shared-panel-heading"><div><Users size={18} /><h2>Most active people</h2></div></div><CountTable label="Person" rows={usage.top_users} valueLabel="Actions" /></section>
      </div>
      <section className="shared-settings-group">
        <div className="shared-panel-heading"><div><Database size={18} /><h2>Workspaces</h2></div><span>{usage.workspaces.length}, largest first</span></div>
        <div className="rm-map-coverage"><table>
          <thead><tr><th scope="col">Workspace</th><th scope="col">Organization</th><th scope="col">Members</th><th scope="col">Files</th><th scope="col">Size</th><th scope="col">Passages</th><th scope="col">Activity</th><th scope="col">AI requests</th><th scope="col">Last activity</th></tr></thead>
          <tbody>{usage.workspaces.map((workspace) => <tr key={workspace.workspace_id}>
            <th scope="row"><button className="rm-system-link" onClick={() => onOpenWorkspace(workspace.workspace_id)} type="button">{workspace.workspace_name}</button></th>
            <td>{workspace.organization_name ?? "—"}</td><td>{count(workspace.member_count)}</td><td>{count(workspace.artifact_count)}</td><td>{formatBytes(workspace.artifact_bytes)}</td><td>{count(workspace.chunk_count)}</td><td>{count(workspace.activity_last_30_days)}</td><td>{count(workspace.ai_requests_last_30_days)}</td><td>{formatTime(workspace.last_activity_at)}</td>
          </tr>)}</tbody>
        </table></div>
      </section>
    </> : <p className="shared-muted-copy">{isLoading ? "Loading usage…" : "Usage is not available."}</p>}
  </>;
}

const ROLE_ICON = { owner: <Crown size={12} />, admin: <ShieldCheck size={12} />, member: <UserIcon size={12} />, viewer: <Eye size={12} /> };

function RoleTag({ role, title }: { role: "owner" | "admin" | "member" | "viewer"; title: string }) {
  return <span className="rm-system-tag rm-system-role" title={title}>{ROLE_ICON[role]} {role[0].toUpperCase() + role.slice(1)}</span>;
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
    {inherited ? <li><span className="rm-system-tag rm-system-role" title="System administrators are administrators of every organization and workspace"><ShieldCheck size={12} /> All organizations and workspaces · Admin</span></li> : null}
    {Array.from(groups, ([key, group]) => <li key={key}>
      <span className="rm-system-access-org"><Building size={14} /><strong>{group.name}</strong>{group.role ? <RoleTag role={group.role} title={`${group.role} of the organization ${group.name}`} /> : null}</span>
      {group.workspaces.length ? <ul>{group.workspaces.map((workspace) => <li key={workspace.workspace_id}><span>{workspace.name}</span><RoleTag role={workspace.role} title={`${workspace.role} of the workspace ${workspace.name}`} /></li>)}</ul> : null}
    </li>)}
  </ul>;
}

type UserAction = { kind: "grant" | "revoke" | "grant-app" | "revoke-app" | "sign-out" | "reset" | "deactivate" | "reactivate" | "anonymize"; user: SystemUser };

function UsersSection({ accessToken, session }: { accessToken: string; session: SharedSession }) {
  const [users, isLoading, reload] = useSystemData<SystemUser[]>(() => listSystemUsers(accessToken), [accessToken]);
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
    } catch (error) { showToast("error", errorMessage(error, "The change could not be made.")); }
    finally { setIsBusy(false); }
  }

  async function unlock(user: SystemUser) {
    try {
      const result = await unlockSystemUser(accessToken, user.id);
      showToast("success", result.cleared ? `Sign-in lockouts lifted for ${user.display_name}.` : `${user.display_name} had no sign-in lockout.`);
    } catch (error) { showToast("error", errorMessage(error, "The lockout could not be lifted.")); }
  }

  const dialog = pending ? {
    grant: { title: `Make ${pending.user.display_name} a system administrator?`, description: "They will see every organization and workspace as an administrator, and manage users, settings and maintenance for the whole server.", action: "Make system administrator" },
    revoke: { title: `Remove ${pending.user.display_name} from the system administrators?`, description: pending.user.id === session.user.id ? "You will lose access to the System pages and to workspaces you are not a member of." : "They keep their own organization and workspace memberships.", action: "Remove role" },
    "grant-app": { title: `Make ${pending.user.display_name} an app administrator?`, description: "They can manage users, settings, logs and maintenance for the whole server, but are not an administrator of every organization and workspace: they only reach the ones they belong to.", action: "Make app administrator" },
    "revoke-app": { title: `Remove ${pending.user.display_name} from the app administrators?`, description: pending.user.id === session.user.id ? "You will lose access to the System pages." : "They keep their own organization and workspace memberships.", action: "Remove role" },
    "sign-out": { title: `Sign ${pending.user.display_name} out everywhere?`, description: "Every device signed in to this account is signed out at once, and its personal access tokens are revoked, so scripts using them stop too. Their password is not changed.", action: "Sign out everywhere" },
    deactivate: { title: `Deactivate ${pending.user.display_name}?`, description: "They can no longer sign in or use a token. Their sessions end now, their API tokens are revoked and the invitations they sent are withdrawn. Everything they wrote stays, and so do their memberships. An owner must transfer what they own first. You can reactivate the account later.", action: "Deactivate account" },
    anonymize: { title: `Anonymize ${pending.user.display_name}?`, description: <>Their email address, name, password, picture, memberships, notices and assistant conversations are removed for good, and the address and name are taken out of activity summaries. What they wrote stays, credited to <strong>Former member</strong>. Log files and earlier backups are not rewritten. <strong>This cannot be undone.</strong> Type <code>{pending.user.email}</code> to confirm.<Input aria-label="Confirm the email address" autoComplete="off" onChange={(event) => setConfirmText(event.target.value)} placeholder={pending.user.email} value={confirmText} /></>, action: "Anonymize account" },
    reactivate: { title: `Reactivate ${pending.user.display_name}?`, description: "They can sign in again with the password they had. Sessions, tokens and invitations that ended when the account was deactivated are not brought back.", action: "Reactivate account" },
    reset: { title: `Create a password reset link for ${pending.user.display_name}?`, description: "You get a one-time link to pass on to them. It works for 24 hours; a newer link replaces it. Their current password keeps working until the link is used; then every device of the account is signed out and sign-in lockouts are lifted.", action: "Create link" },
  }[pending.kind] : null;

  return <>
    <div className="rm-system-toolbar">
      <Input aria-label="Filter users" onChange={(event) => setFilter(event.target.value)} placeholder="Filter by name or email" value={filter} />
      <RefreshButton isLoading={isLoading} onClick={() => void reload()} />
    </div>
    {users ? <div className="rm-map-coverage"><table>
      <thead><tr><th scope="col">Person</th><th scope="col">Role</th><th scope="col">Organizations and workspaces</th><th scope="col">Sessions</th><th scope="col">Last connected</th><th scope="col">Joined</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
      <tbody>{visible.map((user) => <tr className={user.deactivated_at ? "rm-system-row-inactive" : undefined} key={user.id}>
        <th scope="row"><span className="rm-system-person"><UserAvatar name={user.display_name} size={32} userId={user.id} /><span><strong>{user.display_name}{user.id === session.user.id ? " (you)" : ""}</strong><small>{user.email}</small>{user.deactivated_at ? <small><span className="rm-system-tag" title={`Deactivated ${formatTime(user.deactivated_at)}`}><UserOff size={12} /> {user.anonymized_at ? "Anonymized" : "Deactivated"}</span></small> : null}</span></span></th>
        <td>{user.is_system_admin ? <span className="rm-system-tag rm-system-role" title="Administers the server and every organization and workspace"><ServerCog size={13} /> Sys Admin</span> : user.is_app_admin ? <span className="rm-system-tag rm-system-role" title="Uses the System pages, without access to every organization and workspace"><UserCog size={13} /> App Admin</span> : <span className="rm-system-tag rm-system-role"><UserIcon size={13} /> User</span>}</td>
        <td><AccessTree user={user} /></td><td>{count(user.active_sessions)}</td><td>{formatTime(user.last_connected_at)}</td><td>{formatTime(user.created_at)}</td>
        <td>{session.is_system_admin && !user.anonymized_at ? <ActionMenu items={[
          user.is_system_admin
            ? { label: "Remove system administrator role", icon: <ShieldOff size={15} />, onSelect: () => setPending({ kind: "revoke", user }), destructive: true }
            : { label: "Make system administrator", icon: <ShieldCheck size={15} />, onSelect: () => setPending({ kind: "grant", user }) },
          user.is_app_admin
            ? { label: "Remove app administrator role", icon: <ShieldOff size={15} />, onSelect: () => setPending({ kind: "revoke-app", user }), destructive: true }
            : { label: "Make app administrator", icon: <UserCog size={15} />, onSelect: () => setPending({ kind: "grant-app", user }) },
          { label: "Create password reset link", icon: <Key size={15} />, onSelect: () => setPending({ kind: "reset", user }) },
          { label: "Sign out everywhere", icon: <Logout size={15} />, onSelect: () => setPending({ kind: "sign-out", user }), destructive: true },
          { label: "Lift sign-in lockout", icon: <LockOpen size={15} />, onSelect: () => void unlock(user) },
          user.deactivated_at
            ? { label: "Reactivate account", icon: <UserCheck size={15} />, onSelect: () => setPending({ kind: "reactivate", user }) }
            : { label: "Deactivate account", icon: <UserOff size={15} />, onSelect: () => setPending({ kind: "deactivate", user }), destructive: true },
          ...(user.deactivated_at && !user.anonymized_at ? [{ label: "Anonymize account", icon: <UserOff size={15} />, onSelect: () => { setConfirmText(""); setPending({ kind: "anonymize", user }); }, destructive: true }] : []),
        ]} label={`Actions for ${user.display_name}`} /> : null}</td>
      </tr>)}</tbody>
    </table></div> : <p className="shared-muted-copy">{isLoading ? "Loading users…" : "Users are not available."}</p>}
    <Dialog description={dialog?.description} footer={<><DialogCancel onClick={() => setPending(null)} /><Button disabled={isBusy || (pending?.kind === "anonymize" && confirmText.trim().toLowerCase() !== pending.user.email.toLowerCase())} onClick={() => void confirm()} type="button" variant="main">{isBusy ? <Loader className="spin" size={16} /> : null}{dialog?.action}</Button></>} onClose={() => setPending(null)} open={Boolean(pending)} title={dialog?.title ?? ""} />
    {resetLink ? <SecretDialog description={`Send this link to ${resetLink.user.display_name} (${resetLink.user.email}) through a channel you trust. It lets whoever opens it choose the account's password, once, until ${formatTime(resetLink.expiresAt)}.`} onClose={() => setResetLink(null)} title="Password reset link" value={resetLink.url} /> : null}
  </>;
}

/** The environment folder this server is attached to, and the way to detach it (system administrators only). */
function EnvironmentGroup({ accessToken }: { accessToken: string }) {
  const [platform, setPlatform] = useState<{ environment: string | null; folder: string; pinned_environment: string | null } | null>(null);
  const [isConfirming, setIsConfirming] = useState(false);
  const [isBusy, setIsBusy] = useState(false);
  useEffect(() => { getPlatformStatus().then(setPlatform).catch(() => undefined); }, []);

  async function detach() {
    setIsBusy(true);
    try {
      await detachEnvironment(accessToken);
      // Every session ended on the server; start over from the environment menu.
      clearSharedSession();
      window.location.assign("/");
    } catch (error) {
      showToast("error", errorMessage(error, "The environment could not be detached."));
      setIsBusy(false);
    }
  }

  return <section className="shared-settings-group rm-environment-card">
    <div className="shared-panel-heading"><div><Unplug size={18} /><h2>Environment</h2></div><span>{platform?.environment ?? "—"}</span></div>
    <p className="shared-muted-copy">All data of this server lives in <code>{platform ? `${platform.folder}/${platform.environment ?? ""}` : "its environment folder"}</code>. Detaching signs everyone out, stops background work and closes the database. The server then waits for an environment to be chosen again, using the code printed on its console. Nothing is deleted.</p>
    {platform?.pinned_environment
      ? <p className="rm-platform-note" role="note"><AlertCircle size={16} /><span>This server starts with <code>{platform.pinned_environment}</code>, set by <code>REPOMEMO_SERVER_DATA_DIR</code>. Another environment chosen after detaching is used only until the server restarts, then it goes back to <code>{platform.pinned_environment}</code>.</span></p>
      : <p className="rm-platform-note" role="note"><CircleCheck size={16} /><span>This server remembers the environment chosen in the web app and attaches it again when it restarts. After detaching, the next start shows the environment menu until one is chosen.</span></p>}
    <div><Button disabled={isBusy || !platform?.environment} onClick={() => setIsConfirming(true)} type="button" variant="secondary"><Unplug size={16} /> Detach environment</Button></div>
    <Dialog description={<>Everyone, you included, is signed out at once and the server stops serving this environment until an administrator with access to the server console attaches one again.{platform?.pinned_environment ? <> <strong>The switch lasts only until the next restart:</strong> this server starts with <code>{platform.pinned_environment}</code> (<code>REPOMEMO_SERVER_DATA_DIR</code>), so a restart goes back to it whatever is chosen now.</> : <> The environment chosen next is remembered for later restarts.</>}</>} footer={<><DialogCancel onClick={() => setIsConfirming(false)} /><Button disabled={isBusy} onClick={() => void detach()} type="button" variant="main">{isBusy ? <Loader className="spin" size={16} /> : null}Detach environment</Button></>} onClose={() => setIsConfirming(false)} open={isConfirming} title={`Detach ${platform?.environment ?? "the environment"}?`} />
  </section>;
}

function SettingsSection({ accessToken, session }: { accessToken: string; session: SharedSession }) {
  const [settings, setSettings] = useState<SystemSettings | null>(null);
  const [draft, setDraft] = useState<Record<string, boolean | number | string>>({});
  const [isBusy, setIsBusy] = useState(false);
  useEffect(() => {
    getSystemSettings(accessToken).then(setSettings).catch((error) => showToast("error", errorMessage(error, "Settings could not be loaded.")));
  }, [accessToken]);

  const groups = useMemo(() => {
    const byGroup = new Map<string, SystemSetting[]>();
    for (const setting of settings?.settings ?? []) byGroup.set(setting.group, [...(byGroup.get(setting.group) ?? []), setting]);
    return [...byGroup.entries()];
  }, [settings]);
  const changed = Object.keys(draft).length;

  function edit(setting: SystemSetting, value: boolean | number | string) {
    setDraft((current) => {
      const next = { ...current };
      if (value === setting.value) delete next[setting.key];
      else next[setting.key] = value;
      return next;
    });
  }

  async function save() {
    setIsBusy(true);
    try {
      setSettings(await saveSystemSettings(accessToken, draft));
      setDraft({});
      showToast("success", `${changed} setting${changed === 1 ? "" : "s"} saved. They apply now.`);
    } catch (error) { showToast("error", errorMessage(error, "The settings could not be saved.")); }
    finally { setIsBusy(false); }
  }

  async function reset(setting: SystemSetting) {
    setIsBusy(true);
    try {
      setSettings(await resetSystemSetting(accessToken, setting.key));
      setDraft((current) => { const next = { ...current }; delete next[setting.key]; return next; });
      showToast("success", `${setting.label} is back to the server default.`);
    } catch (error) { showToast("error", errorMessage(error, "The setting could not be reset.")); }
    finally { setIsBusy(false); }
  }

  if (!settings) return <p className="shared-muted-copy"><Loader className="spin" size={14} /> Loading settings…</p>;
  // App administrators read the settings; system administrators change them.
  const readOnly = !session.is_system_admin;
  return <>
    {readOnly ? <p className="shared-muted-copy">Read only: only system administrators change these settings.</p> : <div className="rm-system-toolbar"><span className="shared-muted-copy">{changed ? `${changed} unsaved change${changed === 1 ? "" : "s"}` : "No unsaved changes"}</span><div className="rm-system-actions"><Button disabled={!changed || isBusy} onClick={() => setDraft({})} type="button" variant="secondary">Discard</Button><Button disabled={!changed || isBusy} onClick={() => void save()} type="button" variant="main">{isBusy ? <Loader className="spin" size={16} /> : null} Save changes</Button></div></div>}
    {groups.map(([group, entries]) => <section className="shared-settings-group" key={group}>
      <div className="shared-panel-heading"><div><Adjustments size={18} /><h2>{group}</h2></div></div>
      <div className="rm-system-settings">{entries.map((setting) => {
        const value = draft[setting.key] ?? setting.value;
        const inputId = `system-setting-${setting.key}`;
        return <div className="rm-system-setting" key={setting.key}>
          <div><label htmlFor={inputId}><strong>{setting.label}</strong></label><p>{setting.description}</p>
            <small>Server default: {settingValueLabel(setting, setting.default_value)}{setting.overridden ? ` · changed${setting.updated_by ? ` by ${setting.updated_by}` : ""} ${formatTime(setting.updated_at)}` : ""}</small>
          </div>
          <div className="rm-system-setting-control">
            {setting.kind === "boolean"
              ? <label className="shared-ai-provider-toggle"><input checked={Boolean(value)} disabled={readOnly} id={inputId} onChange={(event) => edit(setting, event.target.checked)} type="checkbox" /> {value ? "On" : "Off"}</label>
              : setting.kind === "choice"
                ? <Dropdown aria-label={setting.label} disabled={readOnly} id={inputId} onValueChange={(next) => edit(setting, next)} options={setting.options} value={String(value)} />
                : <span className="rm-system-number"><Input disabled={readOnly} id={inputId} max={setting.max} min={setting.min} onChange={(event) => edit(setting, event.target.value === "" ? setting.min : Number(event.target.value))} type="number" value={String(value)} /><small>{setting.unit}</small></span>}
            {setting.overridden && !readOnly ? <Button disabled={isBusy} onClick={() => void reset(setting)} type="button" variant="secondary"><Rotate size={15} /> Use default</Button> : null}
          </div>
        </div>;
      })}</div>
    </section>)}
    <MailSettingsPanel accessToken={accessToken} canChange={session.is_system_admin === true} />
    {session.is_system_admin ? <EnvironmentGroup accessToken={accessToken} /> : null}
    <section className="shared-settings-group">
      <div className="shared-panel-heading"><div><Server size={18} /><h2>Set by the server environment</h2></div><span>read only</span></div>
      <p className="shared-muted-copy">These are chosen by whoever runs the server, through environment variables, and need a restart to change.</p>
      <Facts items={settings.environment.map((fact) => [fact.label, fact.value])} />
    </section>
  </>;
}

function settingValueLabel(setting: SystemSetting, value: boolean | number | string): string {
  if (setting.kind === "boolean") return value ? "on" : "off";
  if (setting.kind === "choice") return setting.options.find((option) => option.value === value)?.label ?? String(value);
  return `${value}${setting.unit ? ` ${setting.unit}` : ""}`;
}

const LIVE_SOURCE = "live";
const LEVEL_LABELS: Record<string, string> = { off: "off", error: "errors", warn: "warnings", info: "information", debug: "debug", trace: "trace" };

function LogsSection({ accessToken, canDownload, onOpenSettings, workspaces }: { accessToken: string; canDownload: boolean; onOpenSettings: () => void; workspaces: SharedWorkspace[] }) {
  const [source, setSource] = useState(LIVE_SOURCE);
  const [level, setLevel] = useState("info");
  const [category, setCategory] = useState("all");
  const [workspace, setWorkspace] = useState("all");
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
  const [logs, setLogs] = useState<SystemLogs | null>(null);
  const [files, setFiles] = useState<SystemLogFiles | null>(null);
  const isLive = source === LIVE_SOURCE;

  useEffect(() => {
    getSystemLogFiles(accessToken).then(setFiles).catch((error) => showToast("error", errorMessage(error, "The log files could not be listed.")));
  }, [accessToken]);

  useEffect(() => {
    let active = true;
    const load = () => getSystemLogs(accessToken, {
      level,
      category: category === "all" ? undefined : category,
      workspace: workspace === "all" ? undefined : workspace,
      day: isLive ? undefined : source,
      q: query.trim() || undefined,
    })
      .then((next) => { if (active) setLogs(next); })
      .catch((error) => { if (active) showToast("error", errorMessage(error, "Logs could not be loaded.")); });
    const delay = window.setTimeout(() => void load(), query ? 300 : 0);
    const timer = follow && isLive ? window.setInterval(() => void load(), 5_000) : undefined;
    return () => { active = false; window.clearTimeout(delay); if (timer) window.clearInterval(timer); };
  }, [accessToken, level, category, workspace, source, query, follow, isLive]);

  async function download() {
    try {
      const blob = await downloadSystemLogFile(accessToken, source);
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `repomemo-${source}.jsonl`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 10_000);
    } catch (error) { showToast("error", errorMessage(error, "The log file could not be downloaded.")); }
  }

  const sourceOptions = [{ label: "Live: recent events", value: LIVE_SOURCE }, ...(files?.files ?? []).map((file) => ({ label: `${file.day} · ${formatBytes(file.bytes)}`, value: file.day }))];
  const categoryOptions = [{ label: "All sources", value: "all" }, ...(files?.categories ?? []).map((entry) => ({ label: entry.label, value: entry.key })), { label: "Server and other", value: "server" }];
  const workspaceOptions = [{ label: "All workspaces", value: "all" }, ...workspaces.map((entry) => ({ label: entry.workspace.name, value: entry.workspace.id }))];
  return <>
    {files ? <section className="shared-settings-group">
      <div className="shared-panel-heading"><div><Adjustments size={18} /><h2>What is recorded</h2></div><Button onClick={onOpenSettings} type="button" variant="secondary"><Adjustments size={15} /> Logging settings</Button></div>
      {files.environment_filter
        ? <p className="shared-muted-copy">The server's RUST_LOG filter is in force: <code>{files.effective_filter}</code>. Choosing levels in Settings replaces it.</p>
        : <Facts items={[
          ["Server and other", LEVEL_LABELS[files.default_level] ?? files.default_level],
          ...files.categories.map((entry): [string, ReactNode] => [entry.label, LEVEL_LABELS[entry.level] ?? entry.level]),
        ]} />}
      <p className="shared-muted-copy">Console: {files.console_format === "json" ? "JSON lines" : `${files.console_format} text`} · Files: {files.writing_files ? `daily, kept ${files.retention_days ? `${files.retention_days} days` : "forever"}` : "off"}</p>
    </section> : null}
    <div className="rm-system-toolbar">
      <label className="rm-system-inline">Source<Dropdown aria-label="Log source" onValueChange={setSource} options={sourceOptions} value={source} /></label>
      <label className="rm-system-inline">Category<Dropdown aria-label="Log category" onValueChange={setCategory} options={categoryOptions} value={category} /></label>
      <label className="rm-system-inline">Workspace<Dropdown aria-label="Workspace" onValueChange={setWorkspace} options={workspaceOptions} value={workspace} /></label>
      <label className="rm-system-inline">Level<Dropdown aria-label="Lowest level shown" onValueChange={setLevel} options={[{ label: "Errors", value: "error" }, { label: "Warnings and up", value: "warn" }, { label: "Information and up", value: "info" }, { label: "Everything kept", value: "trace" }]} value={level} /></label>
    </div>
    <div className="rm-system-toolbar">
      <Input aria-label="Search logs" onChange={(event) => setQuery(event.target.value)} placeholder="Search message, source or fields" value={query} />
      <div className="rm-system-actions">
        {isLive ? <label className="shared-ai-provider-toggle"><input checked={follow} onChange={(event) => setFollow(event.target.checked)} type="checkbox" /> Follow</label> : canDownload ? <Button onClick={() => void download()} type="button" variant="secondary"><Download size={15} /> Download {source}</Button> : null}
      </div>
    </div>
    {logs && !logs.capturing ? <p className="shared-muted-copy">This server is not capturing logs in memory; read them from its console or log collector.</p> : null}
    {logs?.records.length ? <ol className="rm-system-log" aria-label="Log events, newest first">{logs.records.map((record) => <li className={`level-${record.level}`} key={`${record.sequence}-${record.timestamp}`}>
      <time dateTime={record.timestamp}>{isLive ? new Date(record.timestamp).toLocaleTimeString() : new Date(record.timestamp).toLocaleString()}</time>
      <span className="rm-system-log-level">{record.level}</span>
      <span className="rm-system-log-target">{record.target}</span>
      <span className="rm-system-log-message">{record.message}{record.fields ? <small>{record.fields}</small> : null}</span>
    </li>)}</ol> : <p className="shared-muted-copy">{logs ? "No log events match." : "Loading logs…"}</p>}
  </>;
}

/** Backups of the environment, and restoring one into a new environment folder. */
function BackupsPanel({ accessToken, canChange }: { accessToken: string; canChange: boolean }) {
  const [overview, setOverview] = useState<BackupsOverview | null>(null);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [environment, setEnvironment] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  const wasRunning = useRef(false);
  const running = overview?.status.running ?? false;

  async function load() {
    try { setOverview(await getSystemBackups(accessToken)); }
    catch (error) { showToast("error", errorMessage(error, "Backups could not be listed.")); }
  }
  useEffect(() => { void load(); }, [accessToken]);
  useEffect(() => {
    if (!running) return undefined;
    const timer = window.setInterval(() => void load(), 2_000);
    return () => window.clearInterval(timer);
  }, [running, accessToken]);
  useEffect(() => {
    // Say how a backup or restore ended, once, when it was running before.
    if (wasRunning.current && !running && overview?.status.message) showToast(overview.status.succeeded ? "success" : "error", overview.status.message);
    wasRunning.current = running;
  }, [running, overview]);

  async function backUp() {
    setIsBusy(true);
    try { setOverview(await createSystemBackup(accessToken)); }
    catch (error) { showToast("error", errorMessage(error, "The backup could not start.")); }
    finally { setIsBusy(false); }
  }
  async function copyNow() {
    setIsBusy(true);
    try { setOverview(await copySystemBackups(accessToken)); }
    catch (error) { showToast("error", errorMessage(error, "The copy could not start.")); }
    finally { setIsBusy(false); }
  }
  async function restore() {
    if (!restoring) return;
    setIsBusy(true);
    try { setOverview(await restoreSystemBackup(accessToken, restoring, environment.trim())); setRestoring(null); setEnvironment(""); }
    catch (error) { showToast("error", errorMessage(error, "The restore could not start.")); }
    finally { setIsBusy(false); }
  }
  async function remove() {
    if (!deleting) return;
    setIsBusy(true);
    try { await deleteSystemBackup(accessToken, deleting); setDeleting(null); showToast("success", "The backup was deleted."); await load(); }
    catch (error) { showToast("error", errorMessage(error, "The backup could not be deleted.")); }
    finally { setIsBusy(false); }
  }

  const status = overview?.status;
  return <section className="shared-settings-group">
    <div className="shared-panel-heading"><div><Database size={18} /><h2>Backups</h2></div><span>{running ? `${status?.operation ?? "backup"} running…` : overview ? `${overview.backups.length} kept` : "loading"}</span></div>
    <p className="shared-muted-copy">A backup holds a consistent copy of the database, the stored files and the key file, made while the server runs. It sits in the data folder, so copy it to another machine for it to survive a lost disk, or set REPOMEMO_BACKUP_COPY_DIR and the server copies each backup there and checks the copy. Unless it is encrypted, protect it like the data; with REPOMEMO_BACKUP_KEY set, every file in it is sealed and restoring needs that same key, which is kept nowhere in the backup. Restoring never touches the running environment: it creates a new one, which you attach from the environment menu.</p>
    <Facts items={[
      ["Folder", <code key="dir">{overview?.directory ?? "—"}</code>],
      ["Automatic backups", overview ? (overview.interval_hours ? `every ${overview.interval_hours} h, ${overview.keep} kept` : `off, ${overview.keep} kept (Settings › Automatic backups)`) : "—"],
      ["Second copy", overview ? (overview.copy_directory ? <><code key="copy">{overview.copy_directory}</code>, each backup checked after it is copied</> : "off: set REPOMEMO_BACKUP_COPY_DIR to a folder on another disk or a share") : "—"],
      ["Encryption", overview ? (overview.encryption_enabled ? "on: new backups are sealed with REPOMEMO_BACKUP_KEY" : "off: set REPOMEMO_BACKUP_KEY to seal new backups") : "—"],
      ["Free disk", overview?.disk_free_bytes != null ? formatBytes(overview.disk_free_bytes) : "unknown"],
      ["Last backup or restore", status?.finished_at ? `${formatTime(status.finished_at)} · ${status.message ?? ""}` : "none since the server started"],
    ]} />
    {canChange ? <div><Button disabled={isBusy || running} onClick={() => void backUp()} type="button" variant="main">{running ? <Loader className="spin" size={16} /> : <Database size={16} />} Back up now</Button>{overview?.copy_directory && overview.backups.some((backup) => !backup.copy) ? <Button disabled={isBusy || running} onClick={() => void copyNow()} type="button" variant="secondary">Copy to the second location now</Button> : null}</div> : <p className="shared-muted-copy">Only system administrators make, restore and delete backups.</p>}
    {overview?.backups.length ? <div className="rm-map-coverage"><table>
      <thead><tr><th scope="col">Backup</th><th scope="col">Made</th><th scope="col">Version</th><th scope="col">Files</th><th scope="col">Database</th><th scope="col">Key</th>{overview.copy_directory ? <th scope="col">Second copy</th> : null}{canChange ? <th scope="col"><span className="sr-only">Actions</span></th> : null}</tr></thead>
      <tbody>{overview.backups.map((backup) => <tr key={backup.name}>
        <th scope="row">{backup.name}<small className="rm-system-tag">{backup.trigger}</small>{backup.encryption ? <small className="rm-system-tag">encrypted</small> : null}</th>
        <td>{formatTime(backup.created_at)}</td><td>{backup.version}</td>
        <td>{count(backup.blob_count)} · {formatBytes(backup.blob_bytes)}{backup.missing_blobs ? ` · ${count(backup.missing_blobs)} missing` : ""}</td>
        <td>{formatBytes(backup.database_bytes)}</td><td>{backup.includes_key ? "included" : "not in it"}</td>
        {overview.copy_directory ? <td>{backup.copy ? `checked ${formatTime(backup.copy.copied_at)}` : "waiting"}</td> : null}
        {canChange ? <td><ActionMenu items={[
          { label: "Restore as a new environment…", onSelect: () => { setEnvironment(""); setRestoring(backup.name); } },
          { label: "Delete", onSelect: () => setDeleting(backup.name), destructive: true },
        ]} label={`Actions for ${backup.name}`} /></td> : null}
      </tr>)}</tbody>
    </table></div> : <p className="shared-muted-copy">{overview ? "No backups yet." : "Loading backups…"}</p>}
    <Dialog description="The backup is copied into a new environment folder under workspace-data and checked: it must open. The running environment is not changed. Attach the new one from the environment menu when you want to use it." footer={<><DialogCancel onClick={() => setRestoring(null)} /><Button disabled={isBusy || !environment.trim()} onClick={() => void restore()} type="button" variant="main">{isBusy ? <Loader className="spin" size={16} /> : null}Restore</Button></>} onClose={() => setRestoring(null)} open={Boolean(restoring)} title={`Restore ${restoring ?? "the backup"}`}>
      <label className="rm-system-inline">New environment name<Input aria-label="New environment name" onChange={(event) => setEnvironment(event.target.value)} placeholder="restored-2026-10" value={environment} /></label>
    </Dialog>
    <Dialog description="The backup is deleted from the backups folder. Other backups that share its stored files keep them." footer={<><DialogCancel onClick={() => setDeleting(null)} /><Button disabled={isBusy} onClick={() => void remove()} type="button" variant="main">{isBusy ? <Loader className="spin" size={16} /> : null}Delete backup</Button></>} onClose={() => setDeleting(null)} open={Boolean(deleting)} title={`Delete ${deleting ?? "the backup"}?`} />
  </section>;
}

function JobsSection({ accessToken, canChange }: { accessToken: string; canChange: boolean }) {
  const [status, setStatus] = useState("all");
  // Job changes in every workspace arrive on the System event stream; the
  // list then refreshes only now and then, as a safety net.
  const liveHub = useLiveEventHub("/v1/system/events", accessToken);
  const [jobs, isLoading, reload, setJobs] = useSystemData<SystemJob[]>(() => listSystemJobs(accessToken, status === "all" ? undefined : status), [accessToken, status, liveHub.connected], liveHub.connected ? 60_000 : 10_000);
  const isLive = useLiveEvents((event) => {
    if (event.type === "resync") { void reload(); return; }
    if (event.type !== "job") return;
    const { job } = event;
    const matches = status === "all" || job.status === status;
    const known = jobs?.some((entry) => entry.id === job.id) ?? false;
    const workspaceName = jobs?.find((entry) => entry.workspace_id === job.workspace_id)?.workspace_name;
    // A job of a workspace not listed yet needs its name from the server.
    if (!known && matches && workspaceName === undefined) { void listSystemJobs(accessToken, status === "all" ? undefined : status).then(setJobs).catch(() => undefined); return; }
    setJobs((current) => {
      if (!current) return current;
      const index = current.findIndex((entry) => entry.id === job.id);
      if (index >= 0) return matches ? current.map((entry, position) => position === index ? { ...entry, ...job } : entry) : current.filter((_, position) => position !== index);
      return matches ? [{ ...job, workspace_name: workspaceName ?? null }, ...current].slice(0, 100) : current;
    });
  }, liveHub);
  const [maintenance, setMaintenance] = useState<MaintenanceStatus | null>(null);
  useEffect(() => { getSystemOverview(accessToken).then((overview) => setMaintenance(overview.background.maintenance)).catch(() => undefined); }, [accessToken]);

  async function cancel(job: SystemJob) {
    try { await cancelSharedJob(accessToken, job.id); showToast("success", "Cancellation requested; the job stops at its next step."); await reload(); }
    catch (error) { showToast("error", errorMessage(error, "The job could not be cancelled.")); }
  }

  return <>
    <MaintenancePanel accessToken={accessToken} onChanged={setMaintenance} status={maintenance} />
    <BackupsPanel accessToken={accessToken} canChange={canChange} />
    <section className="shared-settings-group">
      <div className="shared-panel-heading"><div><Activity size={18} /><h2>Jobs</h2></div><span>{isLive ? "newest first · live" : "newest first · refreshes every 10 seconds"}</span></div>
      <div className="rm-system-toolbar">
        <label className="rm-system-inline">Status<Dropdown aria-label="Job status" onValueChange={setStatus} options={[{ label: "All", value: "all" }, { label: "Running", value: "running" }, { label: "Failed", value: "failed" }, { label: "Completed", value: "completed" }, { label: "Cancelled", value: "cancelled" }]} value={status} /></label>
        <RefreshButton isLoading={isLoading} onClick={() => void reload()} />
      </div>
      {jobs?.length ? <div className="rm-map-coverage"><table>
        <thead><tr><th scope="col">Kind</th><th scope="col">Workspace</th><th scope="col">Status</th><th scope="col">Progress</th><th scope="col">Started</th><th scope="col">Updated</th><th scope="col">Detail</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
        <tbody>{jobs.map((job) => <tr key={job.id}>
          <th scope="row">{(job.kind ?? "indexing").replace(/_/g, " ")}</th>
          <td>{job.workspace_name ?? "—"}</td>
          <td>{job.status}{job.cancel_requested && job.status === "running" ? " (stopping)" : ""}</td>
          <td>{job.progress_total ? `${count(job.progress_current)} / ${count(job.progress_total)}` : count(job.progress_current)}</td>
          <td>{formatTime(job.created_at)}</td><td>{formatTime(job.updated_at)}</td>
          <td>{job.error_message ?? job.stage.replace(/_/g, " ")}</td>
          <td>{job.status === "running" && !job.cancel_requested ? <Button onClick={() => void cancel(job)} type="button" variant="secondary">Stop</Button> : null}</td>
        </tr>)}</tbody>
      </table></div> : <p className="shared-muted-copy">{jobs ? "No jobs match." : "Loading jobs…"}</p>}
    </section>
  </>;
}

function AuditSection({ accessToken }: { accessToken: string }) {
  const [events, isLoading, reload] = useSystemData<SystemAuditEvent[]>(() => listSystemAuditEvents(accessToken), [accessToken]);
  return <>
    <div className="rm-system-toolbar"><span className="shared-muted-copy">The 200 most recent events. Sign-in and other security events are under Logs › Security events only.</span><RefreshButton isLoading={isLoading} onClick={() => void reload()} /></div>
    {events?.length ? <div className="rm-map-coverage"><table>
      <thead><tr><th scope="col">When</th><th scope="col">Who</th><th scope="col">Action</th><th scope="col">Detail</th></tr></thead>
      <tbody>{events.map((event) => <tr key={event.id}><td>{formatTime(event.created_at)}</td><td>{event.actor ? event.actor.display_name : "System"}</td><th scope="row">{event.action.replace(/_/g, " ")}</th><td>{event.detail}</td></tr>)}</tbody>
    </table></div> : <p className="shared-muted-copy">{events ? "Nothing recorded yet." : "Loading the audit trail…"}</p>}
  </>;
}
