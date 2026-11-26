import { useState } from "react";
import { DownloadSimple } from "@phosphor-icons/react";
import { getCairnUsage } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnUsageRow } from "../../types";
import { Button } from "../../components/ui/button";
import { Dropdown } from "../../components/ui/dropdown";
import { EmptyState } from "../../components/ui/empty-state";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { useSystemData } from "../system/shared";

const HEADERS = ["Organization", "Workspace", "Model", "People", "Runs", "Tokens", "Failures", "Fallbacks", "Steps per run", "Seconds per run"];

function cells(row: CairnUsageRow): (string | number)[] {
  const figure = (value: number | null, digits = 0) => (value === null ? "" : digits ? value.toFixed(digits) : value);
  return [row.organization, row.workspace, row.model || "no model", row.people, figure(row.runs), figure(row.tokens), figure(row.failures), figure(row.fallbacks), figure(row.average_steps, 1), figure(row.average_seconds, 1)];
}

/** CAIRN's usage and health over 7 or 30 days (V0.9.102): figures only, with no message, chat name or person, and nothing for a group of fewer than five people. */
export function UsagePanel({ accessToken }: { accessToken: string }) {
  const [days, setDays] = useState("7");
  const [usage, isLoading] = useSystemData(cairnAdminKeys.usage(Number(days)), () => getCairnUsage(accessToken, Number(days)), { failure: "The usage could not be loaded." });
  function download() {
    if (!usage) return;
    const escape = (value: string | number) => `"${String(value).replace(/"/g, '""')}"`;
    const lines = [HEADERS, ...[...usage.rows, usage.total].map(cells)].map((line) => line.map(escape).join(","));
    const url = URL.createObjectURL(new Blob([lines.join("\n")], { type: "text/csv" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `cairn-usage-${days}-days.csv`;
    link.click();
    URL.revokeObjectURL(url);
  }
  return <section className="shared-settings-group">
    <div className="rm-system-toolbar">
      <Dropdown aria-label="Period" onValueChange={setDays} options={[{ label: "Last 7 days", value: "7" }, { label: "Last 30 days", value: "30" }]} value={days} />
      <Button disabled={!usage} onClick={download} type="button" variant="secondary"><DownloadSimple /> Export CSV</Button>
    </div>
    {usage ? usage.rows.length ? <Table className="rm-dense-table">
      <thead><tr>{HEADERS.map((header) => <th key={header} scope="col">{header}</th>)}</tr></thead>
      <tbody>{[...usage.rows, usage.total].map((row, index) => <tr key={`${row.organization}-${row.workspace}-${row.model}-${index}`}>
        {cells(row).map((value, column) => column === 0 ? <th key={column} scope="row">{value}</th> : <td key={column}>{value === "" ? <span className="shared-muted-copy">{column === 4 ? "Fewer than 5 people" : "—"}</span> : value}</td>)}
      </tr>)}</tbody>
    </Table> : <EmptyState description="Runs appear here once CAIRN is used." title="No CAIRN run in this period" /> : isLoading ? <SkeletonList label="Loading usage" /> : null}
    <p className="shared-muted-copy">Figures only: no message, chat name or person. A group of fewer than five people shows its size and nothing else.</p>
  </section>;
}
