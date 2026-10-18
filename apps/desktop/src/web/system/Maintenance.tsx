import { useState } from "react";
import { IconLoader2 as Loader, IconRotate as Rotate, IconTool as Tool } from "@tabler/icons-react";
import { runSystemMaintenance } from "../../lib/sharedApi";
import type { MaintenanceStatus } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage, formatBytes, formatTime } from "../format";
import { count, Facts } from "./shared";

export function MaintenancePanel({ accessToken, status, onChanged }: { accessToken: string; status: MaintenanceStatus | null; onChanged: (status: MaintenanceStatus) => void }) {
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
    } catch (error) { showToast("error", apiMessage(error, "Maintenance could not run.")); }
    finally { setIsRunning(false); }
  }
  const report = status?.last_report;
  return <section className="shared-settings-group">
    <PanelHeading description={status?.running ? "running" : status?.next_run_at ? `next ${formatTime(status.next_run_at)}` : "scheduled runs off"} icon={Tool} title="Maintenance" />
    <p className="shared-muted-copy">Clears expired sessions and old jobs, deletes stored files nothing references, retries failed indexing and embeddings, and checkpoints the database.</p>
    <Facts items={[
      ["Last run", status?.last_finished_at ? `${formatTime(status.last_finished_at)} (${status.last_trigger ?? "scheduled"})` : "not yet since the server started"],
      ["Stored files removed", report ? `${count(report.blobs_removed)} · ${formatBytes(report.blob_bytes_reclaimed)}` : "—"],
      ["Cached previews removed", report ? count(report.previews_removed) : "—"],
      ["Old jobs removed", report ? count(report.jobs_pruned) : "—"],
      ["Expired sessions removed", report ? count(report.refresh_tokens_purged) : "—"],
      ["Failed steps", report ? count(report.failures) : "—"],
    ]} />
    <div><Button disabled={isRunning || status?.running} onClick={() => void run()} type="button" variant="main">{isRunning ? <Loader className="spin" size={ICON_SIZE.md} /> : <Rotate size={ICON_SIZE.md} />} Run maintenance now</Button></div>
  </section>;
}
