import { useState } from "react";
import { IconActivity as Activity, IconChartBar as ChartBar, IconDatabase as Database, IconUsers as Users } from "@tabler/icons-react";
import { getSystemUsage } from "../../lib/sharedApi";
import type { SystemUsage } from "../../types";
import { Table } from "../../components/ui/table";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Dropdown } from "../../components/ui/dropdown";
import { LoadingState } from "../../components/ui/loading-state";
import { formatBytes, formatTime } from "../format";
import { useSystemData, count, RefreshButton, BarStrip, CountTable } from "./shared";
import { systemKeys } from "../../lib/queryClient";

export function UsageSection({ accessToken, onOpenWorkspace }: { accessToken: string; onOpenWorkspace: (workspaceId: string) => void }) {
  const [days, setDays] = useState(30);
  const [usage, isLoading, reload] = useSystemData<SystemUsage>(systemKeys.usage(days), () => getSystemUsage(accessToken, days), { keepPrevious: true });
  const total = usage?.activity_by_day.reduce((sum, entry) => sum + entry.count, 0) ?? 0;
  return <>
    <div className="rm-system-toolbar">
      <label className="rm-system-inline">Period<Dropdown aria-label="Usage period" onValueChange={(value) => setDays(Number(value))} options={[{ label: "Last 7 days", value: "7" }, { label: "Last 30 days", value: "30" }, { label: "Last 90 days", value: "90" }, { label: "Last 365 days", value: "365" }]} value={String(days)} /></label>
      <RefreshButton isLoading={isLoading} onClick={() => void reload()} />
    </div>
    {usage ? <>
      <section className="shared-settings-group">
        <PanelHeading description={<>{count(total)} actions</>} icon={ChartBar} title="Recorded activity per day" />
        <BarStrip label={`Recorded activity per day over the last ${usage.days} days`} values={usage.activity_by_day.map((entry) => ({ label: entry.label, value: entry.count }))} />
        <div className="rm-system-axis"><span>{usage.activity_by_day[0]?.label}</span><span>{usage.activity_by_day[usage.activity_by_day.length - 1]?.label}</span></div>
      </section>
      <div className="rm-system-grid">
        <section className="shared-settings-group"><PanelHeading icon={Activity} title="Activity by kind" /><CountTable label="Action" rows={usage.activity_by_action} valueLabel="Times" /></section>
        <section className="shared-settings-group"><PanelHeading icon={Users} title="Most active people" /><CountTable label="Person" rows={usage.top_users} valueLabel="Actions" /></section>
      </div>
      <section className="shared-settings-group">
        <PanelHeading description={<>{usage.workspaces.length}, largest first</>} icon={Database} title="Workspaces" />
        <Table>
          <thead><tr><th scope="col">Workspace</th><th scope="col">Organization</th><th scope="col">Members</th><th scope="col">Files</th><th scope="col">Size</th><th scope="col">Passages</th><th scope="col">Activity</th><th scope="col">AI requests</th><th scope="col">Last activity</th></tr></thead>
          <tbody>{usage.workspaces.map((workspace) => <tr key={workspace.workspace_id}>
            <th scope="row"><button className="rm-system-link" onClick={() => onOpenWorkspace(workspace.workspace_id)} type="button">{workspace.workspace_name}</button></th>
            <td>{workspace.organization_name ?? "—"}</td><td>{count(workspace.member_count)}</td><td>{count(workspace.artifact_count)}</td><td>{formatBytes(workspace.artifact_bytes)}</td><td>{count(workspace.chunk_count)}</td><td>{count(workspace.activity_last_30_days)}</td><td>{count(workspace.ai_requests_last_30_days)}</td><td>{formatTime(workspace.last_activity_at)}</td>
          </tr>)}</tbody>
        </Table>
      </section>
    </> : isLoading ? <LoadingState label="Loading usage" /> : <p className="shared-muted-copy">Usage is not available.</p>}
  </>;
}
