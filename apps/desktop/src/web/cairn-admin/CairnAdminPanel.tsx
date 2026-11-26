import type { ReactNode } from "react";
import { Brain, Buildings, ChartBar, ClockCounterClockwise, Compass, Cpu, ShieldCheck, Sparkle } from "@phosphor-icons/react";
import { getCairnAdmin } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnAdminView } from "../../types";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { EmptyState } from "../../components/ui/empty-state";
import { ICON_SIZE } from "../../components/ui/icon";
import { PageHeader } from "../../components/ui/page-header";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { CAIRN_ADMIN_SECTIONS, type CairnAdminSection } from "../constants";
import { Facts, RefreshButton, useSystemData } from "../system/shared";

export { CAIRN_ADMIN_SECTIONS };
export type { CairnAdminSection };

const SECTION_LABELS: Record<CairnAdminSection, { label: string; icon: ReactNode; intro: string; empty: string }> = {
  overview: { label: "Overview", icon: <Compass size={ICON_SIZE.md} />, intro: "How CAIRN is set up on this server today: each workspace's model and mode, and the budgets and retention in force.", empty: "" },
  models: { label: "Models", icon: <Cpu size={ICON_SIZE.md} />, intro: "The models CAIRN can use, kept once for the whole server.", empty: "The model catalog arrives in a later release. Until then each workspace keeps its own provider in Settings › AI." },
  organizations: { label: "Organizations", icon: <Buildings size={ICON_SIZE.md} />, intro: "Which models each organization uses, and how CAIRN behaves for it.", empty: "Organization settings arrive in a later release." },
  guardrails: { label: "Guardrails", icon: <ShieldCheck size={ICON_SIZE.md} />, intro: "What CAIRN may do, see, send and spend, set once and narrowed by workspaces.", empty: "Guardrails arrive in a later release." },
  abilities: { label: "Abilities", icon: <Sparkle size={ICON_SIZE.md} />, intro: "Procedures an organization or a workspace teaches CAIRN, run with a command.", empty: "Abilities arrive in a later release." },
  memory: { label: "Memory", icon: <Brain size={ICON_SIZE.md} />, intro: "The state of CAIRN's memory, playbooks and routines across workspaces.", empty: "The memory overview arrives in a later release." },
  usage: { label: "Usage", icon: <ChartBar size={ICON_SIZE.md} />, intro: "How much CAIRN is used and how often it fails, in figures only.", empty: "Usage and health arrive in a later release." },
  history: { label: "History", icon: <ClockCounterClockwise size={ICON_SIZE.md} />, intro: "Every change made to how CAIRN is administered.", empty: "The administration history arrives in a later release." },
};

/** Tabs of the CAIRN administration page: only the sections the person may open. */
export function CairnAdminNavigation({ active, onNavigate, sections }: { active: CairnAdminSection; onNavigate: (section: CairnAdminSection) => void; sections: CairnAdminSection[] }) {
  return <nav aria-label="CAIRN administration sections" className="shared-workspace-topbar">
    <div className="shared-workspace-tabs">
      {sections.map((section) => {
        const isActive = section === active;
        return <Button aria-current={isActive ? "page" : undefined} className={isActive ? "active" : ""} key={section} onClick={() => onNavigate(section)} type="button" variant="secondary">{SECTION_LABELS[section].icon}<span className="shared-tab-label">{SECTION_LABELS[section].label}</span></Button>;
      })}
    </div>
  </nav>;
}

/** The sections of the page that a view allows, in the page's order. */
export function allowedSections(view: CairnAdminView | null | undefined): CairnAdminSection[] {
  return CAIRN_ADMIN_SECTIONS.filter((section) => view?.sections.includes(section));
}

function OverviewSection({ view }: { view: CairnAdminView }) {
  const overview = view.overview;
  if (!overview) return <EmptyState description="The server-wide overview is for CAIRN, system and app administrators." title="Nothing to show here" />;
  const { budgets } = overview;
  return <>
    <section className="shared-settings-group">
      <PanelHeading description={`${overview.workspaces.length} workspace${overview.workspaces.length === 1 ? "" : "s"}`} icon={Cpu} title="Model and mode of each workspace" />
      {overview.workspaces.length ? <Table className="rm-dense-table">
        <thead><tr><th scope="col">Organization</th><th scope="col">Workspace</th><th scope="col">Model</th><th scope="col">Provider</th><th scope="col">Mode</th></tr></thead>
        <tbody>{overview.workspaces.map((workspace) => <tr key={workspace.workspace_id}>
          <td>{workspace.organization_name}</td>
          <th scope="row">{workspace.workspace_name}</th>
          <td>{workspace.model ?? <span className="shared-muted-copy">None</span>}</td>
          <td>{workspace.provider_name ? `${workspace.provider_name} · ${workspace.provider_type}` : <span className="shared-muted-copy">No text provider</span>}</td>
          <td>{workspace.mode ? <Badge>{workspace.mode === "agent" ? "Agent" : "Jobs"}</Badge> : <span className="shared-muted-copy">CAIRN answers by its rules</span>}</td>
        </tr>)}</tbody>
      </Table> : <EmptyState title="No workspace yet" />}
    </section>
    <section className="shared-settings-group">
      <PanelHeading description="Set in the server's environment today" icon={ShieldCheck} title="Budgets and retention" />
      <Facts items={[
        ["Steps in a run", String(budgets.max_steps)],
        ["Seconds in a run", String(budgets.max_run_seconds)],
        ["Days a trace is kept", String(budgets.trace_days)],
        ["Files in a background pass", String(budgets.background_max_files)],
        ["Background AI calls per day", budgets.background_ai_per_day.toLocaleString()],
        ["AI requests per person and hour", budgets.ai_requests_per_hour ? budgets.ai_requests_per_hour.toLocaleString() : "Unlimited"],
        ["Models passed by the live evaluation", budgets.eval_passed_models.length ? budgets.eval_passed_models.join(", ") : "None named"],
      ]} />
    </section>
  </>;
}

/** The CAIRN administration page. */
export function CairnAdminPanel({ section, view, isLoading, onReload }: {
  section: CairnAdminSection;
  view: CairnAdminView;
  isLoading: boolean;
  onReload: () => void;
}) {
  const { label, intro, empty } = SECTION_LABELS[section];
  return <section className="shared-page-content rm-system-page">
    <PageHeader actions={<RefreshButton isLoading={isLoading} onClick={onReload} />} description={view.can_change ? intro : `${intro} You can read this page; only CAIRN and system administrators change it.`} title={`CAIRN administration · ${label}`} />
    {section === "overview" ? <OverviewSection view={view} /> : view.scope === "organization" && section === "organizations" ? <section className="shared-settings-group">
      <PanelHeading description={`${view.organizations.length} organization${view.organizations.length === 1 ? "" : "s"}`} icon={Buildings} title="Organizations you administer" />
      <ul>{view.organizations.map((organization) => <li key={organization.id}>{organization.name}</li>)}</ul>
      <p className="shared-muted-copy">{empty}</p>
    </section> : <EmptyState description={empty} icon={SECTION_LABELS[section].icon} title="Nothing here yet" />}
  </section>;
}

/** Loads who the page is for and what it shows. */
export function useCairnAdminView(accessToken: string) {
  return useSystemData<CairnAdminView>(cairnAdminKeys.view(), () => getCairnAdmin(accessToken), { failure: "The CAIRN administration could not be loaded.", silentError: true });
}

export function CairnAdminLoading() {
  return <section className="shared-page-content rm-system-page"><SkeletonList label="Loading the CAIRN administration" /></section>;
}
