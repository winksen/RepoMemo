import { Brain } from "@phosphor-icons/react";
import { getCairnMemory } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import { Badge } from "../../components/ui/badge";
import { EmptyState } from "../../components/ui/empty-state";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { formatTime } from "../format";
import { RefreshButton, useSystemData } from "../system/shared";

/** The state of CAIRN's memory across workspaces (V0.9.112), in figures: cards, those that need review, playbooks and the knowledge workspace. No card is shown. */
export function MemoryPanel({ accessToken }: { accessToken: string }) {
  const [rows, isLoading, reload] = useSystemData(cairnAdminKeys.memory(), () => getCairnMemory(accessToken), { failure: "The memory overview could not be loaded." });
  const total = (pick: (row: NonNullable<typeof rows>[number]) => number) => (rows ?? []).reduce((sum, row) => sum + pick(row), 0);
  return <section className="shared-settings-group">
    <PanelHeading actions={<RefreshButton isLoading={isLoading} onClick={() => void reload()} />} description={rows ? `${total((row) => row.cards).toLocaleString()} cards, ${total((row) => row.needing_review).toLocaleString()} need review, ${total((row) => row.playbooks).toLocaleString()} verified playbooks` : undefined} icon={Brain} title="Memory and playbooks" />
    {rows ? rows.length ? <Table className="rm-dense-table">
      <thead><tr><th scope="col">Organization</th><th scope="col">Workspace</th><th scope="col">Cards</th><th scope="col">Need review</th><th scope="col">Last written</th><th scope="col">Verified playbooks</th></tr></thead>
      <tbody>{rows.map((row) => <tr key={row.workspace_id}>
        <td>{row.organization_name}</td>
        <th scope="row">{row.workspace_name}{row.knowledge ? <> <Badge>Knowledge workspace</Badge></> : null}</th>
        <td>{row.cards.toLocaleString()}</td>
        <td>{row.needing_review ? row.needing_review.toLocaleString() : <span className="shared-muted-copy">0</span>}</td>
        <td>{row.last_written ? formatTime(row.last_written) : <span className="shared-muted-copy">Never</span>}</td>
        <td>{row.playbooks.toLocaleString()}</td>
      </tr>)}</tbody>
    </Table> : <EmptyState description="Workspaces of an organization appear here with the cards and playbooks they hold." title="No workspace yet" /> : isLoading ? <SkeletonList label="Loading the memory overview" /> : null}
  </section>;
}
