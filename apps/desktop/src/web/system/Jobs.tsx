import { useState } from "react";
import { IconActivity as Activity } from "@tabler/icons-react";
import { cancelSharedJob, getSystemOverview, listSystemJobs } from "../../lib/sharedApi";
import type { MaintenanceStatus, SystemJob, SystemOverview } from "../../types";
import { Button } from "../../components/ui/button";
import { Table } from "../../components/ui/table";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Dropdown } from "../../components/ui/dropdown";
import { LoadingState } from "../../components/ui/loading-state";
import { showToast } from "../../components/ui/toast";
import { useLiveEventHub, useLiveEvents } from "../../lib/liveEvents";
import { apiMessage, formatTime } from "../format";
import { useSystemData, count, RefreshButton } from "./shared";
import { MaintenancePanel } from "./Maintenance";
import { BackupsPanel } from "./Backups";
import { systemKeys, invalidateKeys } from "../../lib/queryClient";
import { useServerState } from "../../lib/useServerState";

export function JobsSection({ accessToken, canChange }: { accessToken: string; canChange: boolean }) {
  const [status, setStatus] = useState("all");
  // Job changes in every workspace arrive on the System event stream; the
  // list then refreshes only now and then, as a safety net.
  const liveHub = useLiveEventHub("/v1/system/events", accessToken);
  const [jobs, isLoading, reload, setJobs] = useSystemData<SystemJob[]>(systemKeys.jobs(status), () => listSystemJobs(accessToken, status === "all" ? undefined : status), { keepPrevious: true, refreshMs: liveHub.connected ? 60_000 : 10_000 });
  const isLive = useLiveEvents((event) => {
    if (event.type === "resync") { void invalidateKeys([systemKeys.jobsAll()]); return; }
    if (event.type !== "job") return;
    const { job } = event;
    const matches = status === "all" || job.status === status;
    const known = jobs?.some((entry) => entry.id === job.id) ?? false;
    const workspaceName = jobs?.find((entry) => entry.workspace_id === job.workspace_id)?.workspace_name;
    // A job of a workspace not listed yet needs its name from the server.
    if (!known && matches && workspaceName === undefined) { void invalidateKeys([systemKeys.jobs(status)]); return; }
    setJobs((current) => {
      if (!current) return current;
      const index = current.findIndex((entry) => entry.id === job.id);
      if (index >= 0) return matches ? current.map((entry, position) => position === index ? { ...entry, ...job } : entry) : current.filter((_, position) => position !== index);
      return matches ? [{ ...job, workspace_name: workspaceName ?? null }, ...current].slice(0, 100) : current;
    });
  }, liveHub);
  // The maintenance status is part of the overview, which the Overview page shares through the cache.
  const [overview, setOverview] = useServerState<SystemOverview | null>({ key: systemKeys.overview(), fetch: () => getSystemOverview(accessToken), initial: null, staleTime: 0 });
  const setMaintenance = (next: MaintenanceStatus) => setOverview((current) => current ? { ...current, background: { ...current.background, maintenance: next } } : current);

  async function cancel(job: SystemJob) {
    try { await cancelSharedJob(accessToken, job.id); showToast("success", "Cancellation requested; the job stops at its next step."); await reload(); }
    catch (error) { showToast("error", apiMessage(error, "The job could not be cancelled.")); }
  }

  return <>
    <MaintenancePanel accessToken={accessToken} onChanged={setMaintenance} status={overview?.background.maintenance ?? null} />
    <BackupsPanel accessToken={accessToken} canChange={canChange} />
    <section className="shared-settings-group">
      <PanelHeading description={isLive ? "newest first · live" : "newest first · refreshes every 10 seconds"} icon={Activity} title="Jobs" />
      <div className="rm-system-toolbar">
        <label className="rm-system-inline">Status<Dropdown aria-label="Job status" onValueChange={setStatus} options={[{ label: "All", value: "all" }, { label: "Running", value: "running" }, { label: "Failed", value: "failed" }, { label: "Completed", value: "completed" }, { label: "Cancelled", value: "cancelled" }]} value={status} /></label>
        <RefreshButton isLoading={isLoading} onClick={() => void reload()} />
      </div>
      {jobs?.length ? <Table>
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
      </Table> : jobs ? <p className="shared-muted-copy">No jobs match.</p> : <LoadingState label="Loading jobs" />}
    </section>
  </>;
}
