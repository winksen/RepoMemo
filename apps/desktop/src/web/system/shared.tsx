import { useEffect, useState } from "react";
import type { Dispatch, SetStateAction, ReactNode } from "react";
import { IconRefresh as Refresh } from "@tabler/icons-react";
import type { CountByLabel } from "../../types";
import { Table } from "../../components/ui/table";
import { IconButton } from "../../components/ui/icon-button";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";

/** Loads data, reloads on demand and, optionally, every `refreshMs`. */
export function useSystemData<T>(load: () => Promise<T>, dependencies: unknown[], refreshMs?: number): [T | null, boolean, () => Promise<void>, Dispatch<SetStateAction<T | null>>] {
  const [data, setData] = useState<T | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  async function reload() {
    setIsLoading(true);
    try { setData(await load()); }
    catch (error) { showToast("error", apiMessage(error, "The system information could not be loaded.")); }
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

export function count(value: number): string {
  return value.toLocaleString();
}

export function RefreshButton({ isLoading, onClick }: { isLoading: boolean; onClick: () => void }) {
  return <IconButton busy={isLoading} label="Refresh" onClick={onClick}><Refresh size={ICON_SIZE.md} /></IconButton>;
}

export function Facts({ items }: { items: [string, ReactNode][] }) {
  return <dl className="rm-system-facts">{items.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>;
}

/** A single-series bar strip: one bar per value, newest last, with a hover label per bar. */
export function BarStrip({ label, values }: { label: string; values: { label: string; value: number }[] }) {
  const max = Math.max(1, ...values.map((entry) => entry.value));
  return <div aria-label={label} className="rm-system-bars" role="img">
    {values.map((entry) => <span className="rm-system-bar" key={entry.label} title={`${entry.label}: ${entry.value.toLocaleString()}`}>
      <i style={{ height: `${entry.value ? Math.max(4, (entry.value / max) * 100) : 0}%` }} />
    </span>)}
  </div>;
}

export function CountTable({ rows, label, valueLabel }: { rows: CountByLabel[]; label: string; valueLabel: string }) {
  return rows.length ? <Table>
    <thead><tr><th scope="col">{label}</th><th className="rm-table-number" scope="col">{valueLabel}</th></tr></thead>
    <tbody>{rows.map((row) => <tr key={row.label}><th scope="row">{row.label.replace(/_/g, " ")}</th><td className="rm-table-number">{count(row.count)}</td></tr>)}</tbody>
  </Table> : <p className="shared-muted-copy">Nothing recorded in this period.</p>;
}
