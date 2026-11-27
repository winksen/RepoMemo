import { useState, type ReactNode } from "react";
import { Brain, Buildings, ChartBar, ClockCounterClockwise, Compass, Cpu, ShieldCheck, Sparkle } from "@phosphor-icons/react";
import { getCairnAdmin, updateCairnBudgets } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnAdminView, CairnBudget } from "../../types";
import { Button } from "../../components/ui/button";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { showToast } from "../../components/ui/toast";
import { apiMessage, formatTime } from "../format";
import { Badge } from "../../components/ui/badge";
import { EmptyState } from "../../components/ui/empty-state";
import { ICON_SIZE } from "../../components/ui/icon";
import { PageHeader } from "../../components/ui/page-header";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { CAIRN_ADMIN_SECTIONS, type CairnAdminSection } from "../constants";
import { Facts, RefreshButton, useSystemData } from "../system/shared";
import { ConnectionsPanel } from "./ConnectionsPanel";
import { ModelsPanel } from "./ModelsPanel";
import { UsagePanel } from "./UsagePanel";
import { HistoryPanel } from "./HistoryPanel";
import { AbilitiesPanel } from "./AbilitiesPanel";
import { MemoryPanel } from "./MemoryPanel";
import { OrganizationCairnPanel } from "./OrganizationCairnPanel";
import { Dropdown } from "../../components/ui/dropdown";

export { CAIRN_ADMIN_SECTIONS };
export type { CairnAdminSection };

const SECTION_LABELS: Record<CairnAdminSection, { label: string; icon: ReactNode; intro: string; empty: string }> = {
  overview: { label: "Overview", icon: <Compass size={ICON_SIZE.md} />, intro: "How CAIRN is set up on this server today: each workspace's model and mode, and the budgets and retention in force.", empty: "" },
  models: { label: "Models", icon: <Cpu size={ICON_SIZE.md} />, intro: "The servers CAIRN's models run on, and the models, kept once for the whole server.", empty: "Until the next releases move them, each workspace keeps its own provider in Settings › AI." },
  organizations: { label: "Organizations", icon: <Buildings size={ICON_SIZE.md} />, intro: "Which models each organization uses, and how CAIRN behaves for it.", empty: "Organization settings arrive in a later release." },
  guardrails: { label: "Guardrails", icon: <ShieldCheck size={ICON_SIZE.md} />, intro: "What CAIRN may do, see, send and spend, set once and narrowed by workspaces.", empty: "Guardrails arrive in a later release." },
  abilities: { label: "Abilities", icon: <Sparkle size={ICON_SIZE.md} />, intro: "Procedures an organization or a workspace teaches CAIRN, run with a command.", empty: "Abilities arrive in a later release." },
  memory: { label: "Memory", icon: <Brain size={ICON_SIZE.md} />, intro: "The state of CAIRN's memory, playbooks and routines across workspaces.", empty: "The memory overview arrives in a later release." },
  usage: { label: "Usage", icon: <ChartBar size={ICON_SIZE.md} />, intro: "How much CAIRN is used and how often it fails, in figures only.", empty: "Usage and health arrive in a later release." },
  history: { label: "History", icon: <ClockCounterClockwise size={ICON_SIZE.md} />, intro: "Every change made to how CAIRN is administered.", empty: "The administration history arrives in a later release." },
};

/** The sections of the page that a view allows, in the page's order. */
export function allowedSections(view: CairnAdminView | null | undefined): CairnAdminSection[] {
  return CAIRN_ADMIN_SECTIONS.filter((section) => view?.sections.includes(section));
}

/** The run budgets: one number each, changed together and applied to the next run (V0.9.74). A CAIRN administrator edits; others read. */
function BudgetsPanel({ accessToken, budgets, canChange, onChanged }: { accessToken: string; budgets: CairnBudget[]; canChange: boolean; onChanged: () => void }) {
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [isBusy, setIsBusy] = useState(false);
  const changed = budgets.filter((budget) => drafts[budget.key] !== undefined && drafts[budget.key] !== String(budget.value));
  async function save(change: { values?: Record<string, number>; reset?: string[] }, done: string) {
    setIsBusy(true);
    try {
      await updateCairnBudgets(accessToken, change);
      setDrafts({});
      showToast("success", done);
      onChanged();
    } catch (error) { showToast("error", apiMessage(error, "The budgets could not be changed.")); }
    finally { setIsBusy(false); }
  }
  return <section className="shared-settings-group">
    <PanelHeading description="A change applies to the next run" icon={ShieldCheck} title="Run budgets" />
    <form className="rm-system-budget-form" onSubmit={(event) => {
      event.preventDefault();
      const values: Record<string, number> = {};
      for (const budget of changed) values[budget.key] = Number(drafts[budget.key]);
      void save({ values }, `${changed.length} budget${changed.length === 1 ? "" : "s"} changed. The next run follows.`);
    }}>
      {budgets.map((budget) => <Field hint={`${budget.description} From ${budget.min.toLocaleString()} to ${budget.max.toLocaleString()} ${budget.unit}. Default ${budget.default_value.toLocaleString()}.${budget.overridden && budget.updated_at ? ` Changed ${formatTime(budget.updated_at)}${budget.updated_by ? ` by ${budget.updated_by}` : ""}.` : ""}`} key={budget.key} label={`${budget.label} (${budget.unit})`}>
        <span className="rm-system-budget-row">
          <Input disabled={!canChange} inputMode="numeric" max={budget.max} min={budget.min} onChange={(event) => setDrafts((current) => ({ ...current, [budget.key]: event.target.value }))} type="number" value={drafts[budget.key] ?? String(budget.value)} />
          {canChange && budget.overridden ? <Button busy={isBusy} onClick={() => void save({ reset: [budget.key] }, `${budget.label} is back to ${budget.default_value.toLocaleString()}.`)} type="button" variant="secondary">Restore default</Button> : null}
        </span>
      </Field>)}
      {canChange ? <Button busy={isBusy} disabled={changed.length === 0} type="submit" variant="main">Save budgets</Button> : null}
    </form>
  </section>;
}

/** The Organizations section: choose an organization, then which models it uses and how its workspaces start (V0.9.83). */
function OrganizationsSection({ accessToken, view }: { accessToken: string; view: CairnAdminView }) {
  const [chosen, setChosen] = useState<string>(view.organizations[0]?.id ?? "");
  const organization = view.organizations.find((entry) => entry.id === chosen) ?? view.organizations[0];
  if (!organization) return <EmptyState description="Create an organization first." title="No organization yet" />;
  return <>
    {view.organizations.length > 1 ? <Dropdown aria-label="Organization" onValueChange={setChosen} options={view.organizations.map((entry) => ({ label: entry.name, value: entry.id }))} value={organization.id} /> : null}
    <OrganizationCairnPanel accessToken={accessToken} canChange={view.can_change} key={organization.id} organizationId={organization.id} organizationName={organization.name} />
  </>;
}

/** The Abilities section: choose an organization, then the abilities it teaches CAIRN (V0.9.105). */
function AbilitiesSection({ accessToken, view }: { accessToken: string; view: CairnAdminView }) {
  const [chosen, setChosen] = useState<string>(view.organizations[0]?.id ?? "");
  const organization = view.organizations.find((entry) => entry.id === chosen) ?? view.organizations[0];
  if (!organization) return <EmptyState description="Create an organization first." title="No organization yet" />;
  return <>
    {view.organizations.length > 1 ? <Dropdown aria-label="Organization" onValueChange={setChosen} options={view.organizations.map((entry) => ({ label: entry.name, value: entry.id }))} value={organization.id} /> : null}
    <AbilitiesPanel accessToken={accessToken} canChange={view.can_change} key={organization.id} scope="organization" scopeId={organization.id} scopeName={organization.name} />
  </>;
}

function OverviewSection({ accessToken, onChanged, view }: { accessToken: string; onChanged: () => void; view: CairnAdminView }) {
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
    <BudgetsPanel accessToken={accessToken} budgets={budgets.settings} canChange={view.can_change} key={budgets.settings.map((budget) => `${budget.key}:${budget.value}`).join()} onChanged={onChanged} />
    <section className="shared-settings-group">
      <PanelHeading description="Set in the server's environment" icon={ShieldCheck} title="Other limits" />
      <Facts items={[
        ["Background AI calls per workspace and day", budgets.background_ai_per_day.toLocaleString()],
        ["AI requests per person and hour", budgets.ai_requests_per_hour ? budgets.ai_requests_per_hour.toLocaleString() : "Unlimited"],
        ["Models passed by the live evaluation", budgets.eval_passed_models.length ? budgets.eval_passed_models.join(", ") : "None named"],
      ]} />
    </section>
  </>;
}

/** The CAIRN administration page. */
export function CairnAdminPanel({ accessToken, section, view, isLoading, onReload }: {
  accessToken: string;
  section: CairnAdminSection;
  view: CairnAdminView;
  isLoading: boolean;
  onReload: () => void;
}) {
  const { label, intro, empty } = SECTION_LABELS[section];
  return <section className="shared-page-content rm-system-page">
    <PageHeader actions={<RefreshButton isLoading={isLoading} onClick={onReload} />} description={view.can_change ? intro : `${intro} You can read this page; only CAIRN and system administrators change it.`} title={`CAIRN administration · ${label}`} />
    {section === "memory" ? <MemoryPanel accessToken={accessToken} /> : section === "abilities" ? <AbilitiesSection accessToken={accessToken} view={view} /> : section === "history" ? <HistoryPanel accessToken={accessToken} /> : section === "usage" ? <UsagePanel accessToken={accessToken} /> : section === "organizations" ? <OrganizationsSection accessToken={accessToken} view={view} /> : section === "models" ? <><ConnectionsPanel accessToken={accessToken} canChange={view.can_change} /><ModelsPanel accessToken={accessToken} canChange={view.can_change} /><p className="shared-muted-copy">{empty}</p></> : section === "overview" ? <OverviewSection accessToken={accessToken} onChanged={onReload} view={view} /> : <EmptyState description={empty} icon={SECTION_LABELS[section].icon} title="Nothing here yet" />}
  </section>;
}

/** Loads who the page is for and what it shows. */
export function useCairnAdminView(accessToken: string) {
  return useSystemData<CairnAdminView>(cairnAdminKeys.view(), () => getCairnAdmin(accessToken), { failure: "The CAIRN administration could not be loaded.", silentError: true });
}

export function CairnAdminLoading() {
  return <section className="shared-page-content rm-system-page"><SkeletonList label="Loading the CAIRN administration" /></section>;
}
