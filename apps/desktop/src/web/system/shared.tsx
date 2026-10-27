import { useCallback, useEffect } from "react";
import type { Dispatch, SetStateAction, ReactNode } from "react";
import { IconRefresh as Refresh } from "@tabler/icons-react";
import type { CountByLabel } from "../../types";
import { Table } from "../../components/ui/table";
import { IconButton } from "../../components/ui/icon-button";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { useServerState } from "../../lib/useServerState";
import { apiMessage } from "../format";

/**
 * A System page's data, held in the query cache. It shows the cached answer at once, fetches again when the
 * page opens (and every `refreshMs`), and an answer that arrives after the page was left only reaches the
 * cache, never a page that is gone. Returns the data, whether a fetch is running, a way to fetch again and a
 * setter that writes the cache.
 */
export function useSystemData<T>(key: readonly unknown[], load: () => Promise<T>, options: { refreshMs?: number | ((data: T | null | undefined) => number | false); keepPrevious?: boolean; failure?: string; silentError?: boolean } = {}): [T | null, boolean, () => Promise<void>, Dispatch<SetStateAction<T | null>>, Error | null] {
  const [data, setData, query] = useServerState<T | null>({ key, fetch: load, initial: null, staleTime: 0, refetchInterval: options.refreshMs ?? false, keepPrevious: options.keepPrevious });
  useEffect(() => {
    if (query.error && !options.silentError) showToast("error", apiMessage(query.error, options.failure ?? "The system information could not be loaded."));
  }, [query.error]);
  const reload = useCallback(async () => { await query.refetch(); }, [query.refetch]);
  return [data, query.isFetching, reload, setData, query.error];
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
