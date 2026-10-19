import { useState } from "react";
import { IconDownload as Download, IconLoader2 as Loader } from "@tabler/icons-react";
import { downloadSystemAuditExport, listSystemAuditEvents } from "../../lib/sharedApi";
import type { SystemAuditEvent } from "../../types";
import { Button } from "../../components/ui/button";
import { Table } from "../../components/ui/table";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Dropdown } from "../../components/ui/dropdown";
import { Input } from "../../components/ui/input";
import { LoadingState } from "../../components/ui/loading-state";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage, formatTime } from "../format";
import { useSystemData, count, RefreshButton } from "./shared";
import { systemKeys } from "../../lib/queryClient";

export function AuditSection({ accessToken, canExport }: { accessToken: string; canExport: boolean }) {
  const [events, isLoading, reload] = useSystemData<SystemAuditEvent[]>(systemKeys.audit(), () => listSystemAuditEvents(accessToken));
  const [format, setFormat] = useState<"jsonl" | "csv">("jsonl");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [isExporting, setIsExporting] = useState(false);

  async function exportTrail() {
    setIsExporting(true);
    try {
      const { blob, filename, count } = await downloadSystemAuditExport(accessToken, { format, from, to });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      link.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 10_000);
      showToast("success", `Downloaded ${count.toLocaleString()} audit event${count === 1 ? "" : "s"}. The download is recorded in the trail.`);
      void reload();
    } catch (error) { showToast("error", apiMessage(error, "The audit trail could not be exported.")); }
    finally { setIsExporting(false); }
  }

  return <>
    {canExport ? <section className="shared-settings-group rm-audit-export">
      <PanelHeading description="JSON lines or CSV" icon={Download} title="Export the audit trail" />
      <p className="shared-muted-copy">Every event of a range of days (UTC), oldest first, for an auditor or a log store. Leave both days empty for the whole trail. The export is recorded here, and a CSV cell that a spreadsheet could run as a formula is marked with an apostrophe.</p>
      <div className="rm-system-toolbar">
        <label className="rm-system-inline">Format<Dropdown aria-label="Export format" onValueChange={(value) => setFormat(value === "csv" ? "csv" : "jsonl")} options={[{ label: "JSON lines", value: "jsonl" }, { label: "CSV", value: "csv" }]} value={format} /></label>
        <label className="rm-system-inline">From<Input aria-label="From day" max={to || undefined} onChange={(event) => setFrom(event.target.value)} type="date" value={from} /></label>
        <label className="rm-system-inline">To<Input aria-label="To day" min={from || undefined} onChange={(event) => setTo(event.target.value)} type="date" value={to} /></label>
        <Button disabled={isExporting} onClick={() => void exportTrail()} type="button" variant="secondary">{isExporting ? <Loader className="spin" size={ICON_SIZE.md} /> : <Download size={ICON_SIZE.md} />} Download</Button>
      </div>
    </section> : null}
    <div className="rm-system-toolbar"><span className="shared-muted-copy">The 200 most recent events. Sign-in and other security events are under Logs › Security events only.</span><RefreshButton isLoading={isLoading} onClick={() => void reload()} /></div>
    {events?.length ? <Table>
      <thead><tr><th scope="col">When</th><th scope="col">Who</th><th scope="col">Action</th><th scope="col">Detail</th></tr></thead>
      <tbody>{events.map((event) => <tr key={event.id}><td>{formatTime(event.created_at)}</td><td>{event.actor ? event.actor.display_name : "System"}</td><th scope="row">{event.action.replace(/_/g, " ")}</th><td>{event.detail}</td></tr>)}</tbody>
    </Table> : events ? <p className="shared-muted-copy">Nothing recorded yet.</p> : <LoadingState label="Loading the audit trail" />}
  </>;
}
