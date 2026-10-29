import { type FormEvent, useEffect, useMemo, useState } from "react";
import { IconBuildingCommunity as Building, IconChevronRight as ChevronRight, IconLayoutDashboard as Dashboard, IconLoader2 as Loader, IconPlus as Plus, IconShieldLock as Shield, IconStack2 as Layers } from "@tabler/icons-react";
import { createSharedOrganization, createSharedWorkspace, type DashboardWorkspaceFigures } from "../lib/sharedApi";
import { WorkspaceImport } from "../components/WorkspaceImport";
import { TemplatePicker } from "../components/TemplatePicker";
import { Switch } from "../components/ui/switch";
import { type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { Button } from "../components/ui/button";
import { PageHeader } from "../components/ui/page-header";
import { Input } from "../components/ui/input";
import { Toast, showToast } from "../components/ui/toast";
import { SharedLayout } from "../components/SharedLayout";
import { apiMessage } from "./format";
import { navigate } from "./router";
import { useOrganizationNavigation } from "./navigation";
import { CrossWorkspaceSearch } from "./CrossSearch";
import { OrganizationAdministration, OrganizationRail } from "./Organization";
import { useTemplateCatalogue } from "./workspace/data";
import { cardFigures, headlineText, templateName, useDashboardFigures } from "./dashboardFigures";
import { ICON_SIZE } from "../components/ui/icon";
import { WorkspaceIcon } from "./workspaceIcons";

export function SharedDashboard({
  accessToken,
  apiAvailable,
  organizations,
  onOpenWorkspace,
  session,
  signOut,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  organizations: Organization[];
  onOpenWorkspace: (workspace: SharedWorkspace) => void;
  session: SharedSession;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  const [error, setError] = useState<string | null>(null);
  // The figures come from the query cache: shown at once when the page was open
  // before, refetched on every visit, and again when the workspace list changes.
  const [metricsByWorkspace, , figuresQuery] = useDashboardFigures(accessToken, workspaces);
  const isLoading = figuresQuery.isLoading;
  useEffect(() => { setError(figuresQuery.error ? apiMessage(figuresQuery.error) : null); }, [figuresQuery.error]);

  const summary = useMemo(() => Object.values(metricsByWorkspace).reduce((total, metrics) => ({
    artifacts: total.artifacts + metrics.artifact_count,
    tasks: total.tasks + metrics.active_task_count,
    memory: total.memory + metrics.memory_card_count,
  }), { artifacts: 0, tasks: 0, memory: 0 }), [metricsByWorkspace]);

  return <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizations={organizations} workspaces={workspaces} />}>
    <section className="shared-page-content shared-dashboard-page" aria-busy={isLoading}>
      <PageHeader description="Accessible organizations, workspaces, and the evidence your teams are building." title="Dashboard" />
      <Toast kind="error" message={error} />
      <dl className="shared-dashboard-summary">
        <div><dt>Organizations</dt><dd>{organizations.length}</dd><span>available to your account</span></div>
        <div><dt>Workspaces</dt><dd>{workspaces.length}</dd><span>within your access boundary</span></div>
        <div><dt>Active tasks</dt><dd>{isLoading ? "—" : summary.tasks}</dd><span>open or in progress</span></div>
        <div><dt>Evidence</dt><dd>{isLoading ? "—" : summary.artifacts}</dd><span>stored artifacts</span></div>
        <div><dt>Memory</dt><dd>{isLoading ? "—" : summary.memory}</dd><span>durable team records</span></div>
      </dl>
      {workspaces.length ? <CrossWorkspaceSearch accessToken={accessToken} /> : null}
      <section className="shared-dashboard-ledger" aria-label="Organization workspaces">
        {organizations.length ? organizations.map((organization) => {
          const organizationWorkspaces = workspaces.filter((entry) => entry.organization_id === organization.id);
          const organizationMetrics = organizationWorkspaces.map((entry) => metricsByWorkspace[entry.workspace.id]).filter((metrics): metrics is DashboardWorkspaceFigures => Boolean(metrics));
          const activeTasks = organizationMetrics.reduce((total, metrics) => total + metrics.active_task_count, 0);
          const artifacts = organizationMetrics.reduce((total, metrics) => total + metrics.artifact_count, 0);
          return <section className="shared-dashboard-organization" key={organization.id}>
            <div className="shared-dashboard-organization-heading"><div><span className="shared-dashboard-organization-mark"><Building size={ICON_SIZE.md} /></span><div><h2>{organization.name}</h2><p>{organizationWorkspaces.length} workspace{organizationWorkspaces.length === 1 ? "" : "s"}{organizationMetrics.length ? ` · ${artifacts} artifacts · ${activeTasks} active tasks` : ""}{organization.guest ? " · Guest: only the workspaces you were added to" : ""}</p></div></div><Button onClick={() => navigate(`/workspaces?organization=${encodeURIComponent(organization.id)}`)} type="button" variant="secondary">View workspaces <ChevronRight size={ICON_SIZE.md} /></Button></div>
            {organizationWorkspaces.length ? <div className="shared-dashboard-workspace-list">{organizationWorkspaces.map((entry) => {
              const metrics = metricsByWorkspace[entry.workspace.id];
              return <Button className="shared-dashboard-workspace" key={entry.workspace.id} onClick={() => onOpenWorkspace(entry)} type="button" variant="secondary"><span className="shared-workspace-icon"><WorkspaceIcon size={ICON_SIZE.md} workspace={entry.workspace} /></span><span><strong>{entry.workspace.name}</strong><small>{entry.role} access{templateName(metrics) ? ` · ${templateName(metrics)}` : ""} · updated {new Date(entry.workspace.updated_at).toLocaleDateString()}</small></span><span className="shared-dashboard-workspace-metrics">{metrics ? cardFigures(metrics).map((figure) => figure.headline ? <b key={figure.key}>{figure.text}</b> : <span key={figure.key}>{figure.text}</span>) : <span>Loading workspace data</span>}</span><ChevronRight size={ICON_SIZE.md} /></Button>;
            })}</div> : <p className="shared-muted-copy">No accessible workspaces in this organization yet.</p>}
          </section>;
        }) : <div className="shared-empty-state"><Building size={26} /><strong>No organizations yet</strong>{session.can_create_organizations === false ? <span>Ask a system administrator to create one or to invite you to one.</span> : <><span>Create an organization to start a shared workspace boundary.</span><Button onClick={() => navigate("/workspaces?createOrganization=1")} type="button" variant="main"><Plus size={ICON_SIZE.md} /> Create organization</Button></>}</div>}
      </section>
    </section>
  </SharedLayout>;
}

export function SharedWorkspaceHome({
  accessToken,
  apiAvailable,
  organizations,
  session,
  setError,
  setOrganizations,
  setWorkspaces,
  signOut,
  onOpenWorkspace,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  organizations: Organization[];
  session: SharedSession;
  setError: (value: string | null) => void;
  setOrganizations: (value: Organization[] | ((current: Organization[]) => Organization[])) => void;
  setWorkspaces: (value: SharedWorkspace[] | ((current: SharedWorkspace[]) => SharedWorkspace[])) => void;
  signOut: () => void;
  onOpenWorkspace: (workspace: SharedWorkspace) => void;
  workspaces: SharedWorkspace[];
}) {
  const [organizationName, setOrganizationName] = useState("");
  const [workspaceName, setWorkspaceName] = useState("");
  const [restrictedWorkspace, setRestrictedWorkspace] = useState(false);
  const [templateId, setTemplateId] = useState("general");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const organizationNavigation = useOrganizationNavigation();
  const organizationId = organizations.some((organization) => organization.id === organizationNavigation.activeOrganizationId)
    ? organizationNavigation.activeOrganizationId!
    : organizations[0]?.id ?? "";
  const showOrganizationForm = organizationNavigation.isCreatingOrganization || organizations.length === 0;

  const currentOrganization = useMemo(
    () => organizations.find((organization) => organization.id === organizationId),
    [organizationId, organizations],
  );
  const canManageOrganization = currentOrganization?.role === "owner" || currentOrganization?.role === "admin";
  const [templates] = useTemplateCatalogue(accessToken, canManageOrganization);
  // The same request as the dashboard's, so the list below can say which template each workspace uses.
  const [figures] = useDashboardFigures(accessToken, workspaces);

  async function createOrganization(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setFormError(null); setError(null);
    try {
      const organization = await createSharedOrganization(accessToken, organizationName);
      setOrganizations((current) => [...current, organization]);
      organizationNavigation.selectOrganization(organization.id);
      setOrganizationName("");
      showToast("success", `${organization.name} was created.`);
    } catch (requestError) { setFormError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function createWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!organizationId) return;
    setIsSubmitting(true); setFormError(null); setError(null);
    try {
      const workspace = await createSharedWorkspace(accessToken, organizationId, workspaceName, templates.length > 1 ? templateId : undefined, restrictedWorkspace);
      setWorkspaces((current) => [workspace, ...current]);
      setWorkspaceName("");
      setRestrictedWorkspace(false);
      setTemplateId("general");
      showToast("success", `${workspace.workspace.name} was created.`);
    } catch (requestError) { setFormError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
    <SharedLayout
      apiAvailable={apiAvailable}
      onNavigate={navigate}
      session={session}
      signOut={signOut}
      sidebar={<OrganizationRail organizations={organizations} organizationId={organizationId} workspaces={workspaces} />}
    >
        <div className="shared-page-content">
          <PageHeader actions={organizations.length && session.can_create_organizations !== false ? <Button onClick={organizationNavigation.startCreatingOrganization} type="button" variant="secondary"><Plus size={ICON_SIZE.md} /> New organization</Button> : null} description={showOrganizationForm ? "Create a separate team boundary for its own shared workspaces and members." : "Workspaces are server-authoritative and available only to their members."} title={showOrganizationForm ? "Create an organization" : currentOrganization ? currentOrganization.name : "Set up your team"} />
          {showOrganizationForm ? <form className="shared-setup-form" onSubmit={createOrganization}><label>Organization name<Input autoFocus onChange={(event) => setOrganizationName(event.target.value)} placeholder="Engineering" required value={organizationName} /></label><div className="shared-setup-actions"><Button disabled={isSubmitting} type="submit" variant="main">{isSubmitting ? <Loader className="spin" size={ICON_SIZE.md} /> : <Plus size={ICON_SIZE.md} />} Create organization</Button>{organizations.length ? <Button disabled={isSubmitting} onClick={organizationNavigation.cancelOrganizationCreation} type="button" variant="secondary">Cancel</Button> : null}</div></form> : <>
            {canManageOrganization ? <form className="shared-create-workspace" onSubmit={createWorkspace}><div><strong>Create a workspace</strong><span>Start a bounded memory space for a repository, system, or initiative.</span></div><label className="sr-only" htmlFor="shared-workspace-name">Workspace name</label><Input id="shared-workspace-name" onChange={(event) => setWorkspaceName(event.target.value)} placeholder="Payments platform" required value={workspaceName} /><Button disabled={isSubmitting} type="submit" variant="main">{isSubmitting ? <Loader className="spin" size={ICON_SIZE.md} /> : <Plus size={ICON_SIZE.md} />} Create workspace</Button><TemplatePicker disabled={isSubmitting} name="dashboard-template" onChange={setTemplateId} templates={templates} value={templateId} /><Switch checked={restrictedWorkspace} disabled={isSubmitting} label="Restricted: add people one by one, not the whole organization" onCheckedChange={setRestrictedWorkspace} /></form> : <p className="shared-muted-copy">Your organization role grants access to its workspaces but not workspace creation.</p>}
            {canManageOrganization ? <WorkspaceImport accessToken={accessToken} key={`import-${organizationId}`} onImported={(workspace) => setWorkspaces((current) => [workspace, ...current])} organizationId={organizationId} /> : null}
            <div className="shared-workspace-list">{workspaces.filter((workspace) => workspace.organization_id === organizationId).map((entry) => <article className="shared-workspace-row" key={entry.workspace.id}><span className="shared-workspace-icon"><WorkspaceIcon size={ICON_SIZE.lg} workspace={entry.workspace} /></span><div><h2>{entry.workspace.name}</h2><p>Created {new Date(entry.workspace.created_at).toLocaleDateString()} · Your role: {entry.role}{templateName(figures[entry.workspace.id]) ? ` · ${templateName(figures[entry.workspace.id])}` : ""}{headlineText(figures[entry.workspace.id]?.template) ? ` · ${headlineText(figures[entry.workspace.id]?.template)}` : ""}</p></div><Button aria-label={`Open ${entry.workspace.name}`} className="shared-workspace-open" onClick={() => onOpenWorkspace(entry)} type="button" variant="secondary"><ChevronRight size={ICON_SIZE.lg} /></Button></article>)}{workspaces.filter((workspace) => workspace.organization_id === organizationId).length === 0 ? <div className="shared-empty-state"><Layers size={26} /><strong>No workspaces yet</strong><span>Create the first shared workspace for {currentOrganization?.name}.</span></div> : null}</div>
            {currentOrganization && canManageOrganization ? <OrganizationAdministration accessToken={accessToken} onOrganizationUpdated={(updated) => setOrganizations((current) => current.map((organization) => organization.id === updated.id ? updated : organization))} organization={currentOrganization} /> : null}
          </>}
          <Toast kind="error" message={formError} />
          <p className="shared-boundary-note"><Shield size={ICON_SIZE.sm} /> Every workspace view is loaded through the JWT-protected API. This browser never falls back to local preview data.</p>
        </div>
    </SharedLayout>
  );
}
