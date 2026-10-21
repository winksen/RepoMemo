import { IconActivity as Activity, IconDatabase as Database, IconServer as Server, IconShieldCheck as ShieldCheck, IconUsers as Users, IconWorld as World } from "@tabler/icons-react";
import { getSystemOverview } from "../../lib/sharedApi";
import type { MaintenanceStatus, SystemOverview } from "../../types";
import { Badge } from "../../components/ui/badge";
import { Table } from "../../components/ui/table";
import { PanelHeading } from "../../components/ui/panel-heading";
import { LoadingState } from "../../components/ui/loading-state";
import { formatBytes, formatDuration, formatTime } from "../format";
import { useSystemData, count, RefreshButton, Facts, BarStrip } from "./shared";
import { MaintenancePanel } from "./Maintenance";
import { systemKeys } from "../../lib/queryClient";

/** What System › Overview says about the Office previews: working, off, refused or absent. */
function officePreviewsText(previews: SystemOverview["background"]["office_previews"]): string {
  if (previews.available) return `${previews.version ?? "version not read yet"} · ${previews.sandboxed ? "in a sandbox" : "no sandbox"}`;
  if (previews.refused) return `refused: ${previews.refused}`;
  if (!previews.installed) return previews.requested ? "turned on, but not found" : "not installed";
  return previews.requested ? "turned on, release being checked" : "installed, previews off (REPOMEMO_OFFICE_PREVIEWS)";
}

export function OverviewSection({ accessToken }: { accessToken: string }) {
  const [overview, isLoading, reload, setOverview] = useSystemData<SystemOverview>(systemKeys.overview(), () => getSystemOverview(accessToken), { refreshMs: 30_000 });
  const setMaintenance = (next: MaintenanceStatus) => setOverview((current) => current ? { ...current, background: { ...current.background, maintenance: next } } : current);
  if (!overview) return isLoading ? <LoadingState label="Loading the system overview" /> : <p className="shared-muted-copy">The overview is not available.</p>;
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
        <PanelHeading description={<>up {formatDuration(instance.uptime_seconds)}</>} icon={Server} title="Back end" />
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
        <PanelHeading description={formatBytes(storage.database_bytes + storage.write_ahead_log_bytes + storage.blob_bytes + storage.preview_bytes)} icon={Database} title="Storage" />
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
        <PanelHeading description={background.indexing_queued + background.repository_syncs_running ? "busy" : "idle"} icon={Activity} title="Background work" />
        <Facts items={[
          ["Files waiting for indexing", count(background.indexing_queued)],
          ["Workspaces waiting for vectors", count(background.embedding_workspaces_waiting)],
          ["Repository syncs", count(background.repository_syncs_running)],
          ["Preview conversions", count(background.preview_conversions_running)],
          ["LibreOffice", officePreviewsText(background.office_previews)],
          ["Jobs", statistics.jobs_by_status.map((entry) => `${count(entry.count)} ${entry.label}`).join(" · ") || "none"],
          ["Live event streams", `${count(background.open_event_streams)} open · ${count(background.event_channels)} workspaces watched`],
          ["AI providers", `${count(statistics.enabled_provider_count)} enabled · ${count(statistics.cloud_provider_count)} cloud`],
        ]} />
      </section>
      <section className="shared-settings-group">
        <PanelHeading description="since start" icon={ShieldCheck} title="Traffic and protection" />
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
      <PanelHeading description="seen in the last 24 hours" icon={World} title="Front-end clients" />
      {overview.clients.length ? <Table>
        <thead><tr><th scope="col">Client</th><th scope="col">Origin</th><th scope="col">Last address</th><th scope="col">Requests</th><th scope="col">First seen</th><th scope="col">Last seen</th></tr></thead>
        <tbody>{overview.clients.map((client) => <tr key={`${client.origin}-${client.client}-${client.agent}`}>
          <th scope="row">{client.agent}{client.client ? <Badge>{client.client}</Badge> : null}</th>
          <td>{client.origin ?? "no origin (tool or direct link)"}</td><td><code>{client.last_address}</code></td><td>{count(client.requests)}</td><td>{formatTime(client.first_seen_at)}</td><td>{formatTime(client.last_seen_at)}</td>
        </tr>)}</tbody>
      </Table> : <p className="shared-muted-copy">No clients seen yet.</p>}
    </section>
    <MaintenancePanel accessToken={accessToken} onChanged={setMaintenance} status={background.maintenance} />
  </>;
}
