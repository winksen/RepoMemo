import { useState } from "react";
import { CalendarCheck, Pause, Play } from "@phosphor-icons/react";
import { getCairnRoutines, pauseCairnRoutine, resumeCairnRoutine } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import { ActionMenu } from "../../components/ui/action-menu";
import { Badge } from "../../components/ui/badge";
import { EmptyState } from "../../components/ui/empty-state";
import { ICON_SIZE } from "../../components/ui/icon";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { showToast } from "../../components/ui/toast";
import { apiMessage, formatTime } from "../format";
import { RefreshButton, useSystemData } from "../system/shared";

const RUNS: Record<string, string> = { daily: "Every day", weekly: "Every week", monthly: "Every month", files_indexed: "When files are indexed", file_outdated: "When a file goes out of date" };

/**
 * The routines of the organizations (V0.9.113): who owns each, how it runs, whether it is paused and why, and how it last went. An
 * administrator can pause one and resume one. The prompt is never shown here. A routine that an organization's limits no longer allow is
 * paused again the next time its settings are saved.
 */
export function RoutinesPanel({ accessToken, canChange }: { accessToken: string; canChange: boolean }) {
  const [rows, isLoading, reload] = useSystemData(cairnAdminKeys.routines(), () => getCairnRoutines(accessToken), { failure: "The routines could not be loaded." });
  const [, setBusy] = useState<string | null>(null);

  async function change(id: string, title: string, pause: boolean) {
    setBusy(id);
    try {
      if (pause) await pauseCairnRoutine(accessToken, id); else await resumeCairnRoutine(accessToken, id);
      showToast("success", pause ? `${title} is paused.` : `${title} is resumed.`);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, pause ? "The routine could not be paused." : "The routine could not be resumed.")); }
    finally { setBusy(null); }
  }

  const paused = (rows ?? []).filter((row) => row.state === "paused").length;
  return <section className="shared-settings-group">
    <PanelHeading actions={<RefreshButton isLoading={isLoading} onClick={() => void reload()} />} description={rows ? `${rows.length} routine${rows.length === 1 ? "" : "s"}, ${paused} paused` : undefined} icon={CalendarCheck} title="Routines" />
    {rows ? rows.length ? <Table className="rm-dense-table">
      <thead><tr><th scope="col">Organization</th><th scope="col">Workspace</th><th scope="col">Routine</th><th scope="col">Owner</th><th scope="col">Runs</th><th scope="col">State</th><th scope="col">Last run</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
      <tbody>{rows.map((row) => <tr key={row.id}>
        <td>{row.organization_name}</td>
        <td>{row.workspace_name}</td>
        <th scope="row">{row.title}</th>
        <td>{row.owner_name}</td>
        <td>{RUNS[row.runs] ?? row.runs}</td>
        <td>{row.state === "paused" ? <><Badge>Paused</Badge>{row.pause_reason ? <small className="shared-muted-copy"> {row.pause_reason}</small> : null}</> : <>Active{row.failures ? <small className="shared-muted-copy"> · {row.failures} failed in a row</small> : null}</>}</td>
        <td>{row.last_run_at ? formatTime(row.last_run_at) : <span className="shared-muted-copy">Never</span>}</td>
        <td>{canChange ? <ActionMenu items={[row.state === "paused"
          ? { label: "Resume", icon: <Play size={ICON_SIZE.sm} />, onSelect: () => void change(row.id, row.title, false) }
          : { label: "Pause", icon: <Pause size={ICON_SIZE.sm} />, onSelect: () => void change(row.id, row.title, true) }]} label={`Actions for ${row.title}`} /> : null}</td>
      </tr>)}</tbody>
    </Table> : <EmptyState description="Routines that people save in a workspace appear here." title="No routine yet" /> : isLoading ? <SkeletonList label="Loading the routines" /> : null}
  </section>;
}
