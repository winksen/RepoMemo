import { getCairnHistory } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import { EmptyState } from "../../components/ui/empty-state";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { formatTime } from "../format";
import { RefreshButton, useSystemData } from "../system/shared";

/** Every change made to how CAIRN is administered (V0.9.103): who, when and what changed, with the values before and after. Keys never appear. The System audit trail has the same events. */
export function HistoryPanel({ accessToken }: { accessToken: string }) {
  const [events, isLoading, reload] = useSystemData(cairnAdminKeys.history(), () => getCairnHistory(accessToken), { failure: "The history could not be loaded." });
  return <section className="shared-settings-group">
    <div className="rm-system-toolbar"><RefreshButton isLoading={isLoading} onClick={() => void reload()} /></div>
    {events ? events.length ? <Table className="rm-dense-table">
      <thead><tr><th scope="col">When</th><th scope="col">Who</th><th scope="col">Change</th><th scope="col">What changed</th></tr></thead>
      <tbody>{events.map((event) => <tr key={event.id}>
        <th scope="row">{formatTime(event.at)}</th><td>{event.who}</td><td><code>{event.action}</code></td><td>{event.detail}</td>
      </tr>)}</tbody>
    </Table> : <EmptyState description="Changes to connections, models, budgets and the organization and workspace settings appear here." title="Nothing has been changed yet" /> : isLoading ? <SkeletonList label="Loading the history" /> : null}
  </section>;
}
