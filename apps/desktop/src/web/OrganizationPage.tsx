import { Pulse as Activity, SquaresFour as Dashboard, Gear as Settings, Stack as Layers, type Icon as IconComponent } from "@phosphor-icons/react";
import { Button } from "../components/ui/button";
import { PanelHeading } from "../components/ui/panel-heading";
import { Skeleton } from "../components/ui/skeleton";
import { Table } from "../components/ui/table";
import { getSharedOrganizationOverview } from "../lib/sharedApi";
import { accountKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import { type Organization, type OrganizationOverview, type WorkspaceMetricBreakdown } from "../types";
import { ICON_SIZE } from "../components/ui/icon";
import { MetricBars, MetricTimeline } from "./common";
import { useTabIndicator } from "./tabIndicator";
import { ORGANIZATION_SECTIONS, type OrganizationSection } from "./constants";

const TABS: Record<OrganizationSection, { label: string; icon: IconComponent }> = {
  overview: { label: "Overview", icon: Dashboard },
  workspaces: { label: "Workspaces", icon: Layers },
  administration: { label: "Administration", icon: Settings },
};

/** The tabs an organization's page offers: Administration is for its owners and administrators. */
export function organizationSections(organization: Organization): OrganizationSection[] {
  const administers = organization.role === "owner" || organization.role === "admin";
  return ORGANIZATION_SECTIONS.filter((section) => section !== "administration" || administers);
}

/** The row of tabs of an organization's page, in the form of a workspace's. */
export function OrganizationTopbar({ activeSection, onNavigate, organization }: { activeSection: OrganizationSection; onNavigate: (section: OrganizationSection) => void; organization: Organization }) {
  const { ref: tabsRef, indicator } = useTabIndicator();
  return <nav aria-label="Organization sections" className="shared-workspace-topbar">
    <div className="shared-workspace-tabs" ref={tabsRef}>
      {organizationSections(organization).map((section) => {
        const Icon = TABS[section].icon;
        const isActive = section === activeSection;
        return <Button aria-current={isActive ? "page" : undefined} className={isActive ? "active" : ""} key={section} onClick={() => onNavigate(section)} type="button" variant="secondary"><span className="shared-workspace-tab-label"><Icon size={ICON_SIZE.md} />{TABS[section].label}</span></Button>;
      })}
      {indicator}
    </div>
  </nav>;
}

/** Workspaces as bars, largest first, by one of their figures. */
export function barsBy(overview: OrganizationOverview, figure: "artifact_count" | "memory_card_count" | "active_task_count" | "activity_count"): WorkspaceMetricBreakdown[] {
  return overview.workspaces
    .map((workspace) => ({ label: workspace.name, value: workspace[figure] ?? 0 }))
    .filter((item) => item.value > 0)
    .sort((left, right) => right.value - left.value);
}

/** The Overview tab: totals, how the figures divide between the workspaces, recent activity and a row for each workspace. */
export function OrganizationOverviewTab({ accessToken, onOpenWorkspace, organization }: { accessToken: string; onOpenWorkspace: (workspaceId: string) => void; organization: Organization }) {
  const [overview, , query] = useServerState<OrganizationOverview | null>({
    key: accountKeys.organizationOverview(organization.id), fetch: () => getSharedOrganizationOverview(accessToken, organization.id), initial: null, staleTime: 15_000,
  });
  const loading = overview === null && query.isLoading;
  const figure = (value: number | undefined) => loading || value === undefined ? <Skeleton height={22} width={48} /> : value.toLocaleString();
  const hasActivity = Boolean(overview?.activity_by_day.length);
  return <section aria-busy={loading} className="shared-organization-overview" aria-label="Organization overview">
    <dl className="shared-dashboard-summary">
      <div><dt>Workspaces</dt><dd>{figure(overview?.workspace_count)}</dd><span>you can open</span></div>
      <div><dt>People</dt><dd>{figure(overview?.member_count)}</dd><span>in the organization</span></div>
      <div><dt>Evidence</dt><dd>{figure(overview?.artifact_count)}</dd><span>stored artifacts</span></div>
      <div><dt>Memory</dt><dd>{figure(overview?.memory_card_count)}</dd><span>durable team records</span></div>
      <div><dt>Active tasks</dt><dd>{figure(overview?.active_task_count)}</dd><span>open or in progress</span></div>
    </dl>
    {query.isError && !overview ? <p className="shared-muted-copy">The overview could not be loaded. Try the tab again in a moment.</p> : null}
    {overview ? <>
      <div className="shared-metric-visuals">
        <MetricBars emptyLabel="Evidence appears when a workspace has files." items={barsBy(overview, "artifact_count")} summary={`${overview.artifact_count.toLocaleString()} in all`} title="Evidence by workspace" />
        <MetricBars emptyLabel="Memory cards appear when a workspace keeps some." items={barsBy(overview, "memory_card_count")} summary={`${overview.memory_card_count.toLocaleString()} in all`} title="Memory by workspace" />
        <MetricBars emptyLabel="Open tasks appear here." items={barsBy(overview, "active_task_count")} summary={`${overview.active_task_count.toLocaleString()} open`} title="Active tasks by workspace" />
        {hasActivity ? <MetricTimeline items={overview.activity_by_day} title="Activity, last 30 days" /> : null}
        {hasActivity ? <MetricBars emptyLabel="No recent activity." items={barsBy(overview, "activity_count")} summary="last 30 days" title="Activity by workspace" /> : null}
      </div>
      <section className="shared-organization-overview-table">
        <PanelHeading description={`${overview.workspaces.length} ${overview.workspaces.length === 1 ? "workspace" : "workspaces"}`} icon={Activity} title="By workspace" />
        {overview.workspaces.length ? <Table>
          <thead><tr><th scope="col">Workspace</th><th className="rm-table-number" scope="col">Evidence</th><th className="rm-table-number" scope="col">Memory</th><th className="rm-table-number" scope="col">Active tasks</th>{hasActivity ? <th className="rm-table-number" scope="col">Activity, 30 days</th> : null}</tr></thead>
          <tbody>{overview.workspaces.map((workspace) => <tr key={workspace.workspace_id}>
            <th scope="row"><Button className="shared-link-button" onClick={() => onOpenWorkspace(workspace.workspace_id)} type="button" variant="secondary">{workspace.name}</Button></th>
            <td className="rm-table-number">{workspace.artifact_count.toLocaleString()}</td>
            <td className="rm-table-number">{workspace.memory_card_count.toLocaleString()}</td>
            <td className="rm-table-number">{workspace.active_task_count.toLocaleString()}</td>
            {hasActivity ? <td className="rm-table-number">{workspace.activity_count === null ? "—" : workspace.activity_count.toLocaleString()}</td> : null}
          </tr>)}</tbody>
        </Table> : <p className="shared-muted-copy">No workspaces yet. Use New workspace to create the first one.</p>}
      </section>
    </> : null}
  </section>;
}
