import { IconBuildingCommunity as Building, IconChevronRight as ChevronRight, IconPlus as Plus } from "@tabler/icons-react";
import { SharedLayout } from "../components/SharedLayout";
import { Button } from "../components/ui/button";
import { ICON_SIZE } from "../components/ui/icon";
import { PageHeader } from "../components/ui/page-header";
import { type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { useOrganizationNavigation } from "./navigation";
import { navigate } from "./router";
import { OrganizationRail } from "./Organization";

/** `/orgs`: every organization the account belongs to, with its role and workspace count; each opens its administration. */
export function SharedOrganizations({
  apiAvailable,
  organizations,
  session,
  signOut,
  workspaces,
}: {
  apiAvailable: boolean | null;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  const organizationNavigation = useOrganizationNavigation();
  const canCreate = session.can_create_organizations !== false;
  const create = canCreate ? <Button onClick={organizationNavigation.startCreatingOrganization} type="button" variant="secondary"><Plus size={ICON_SIZE.md} /> New organization</Button> : null;
  return <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizations={organizations} workspaces={workspaces} />}>
    <section className="shared-page-content shared-dashboard-page">
      <PageHeader actions={organizations.length ? create : null} description="Every organization you belong to. Open one to manage its members, templates and workspaces." title="Organizations" />
      {organizations.length ? <div className="shared-dashboard-workspace-list">{organizations.map((organization) => {
        const count = workspaces.filter((workspace) => workspace.organization_id === organization.id).length;
        return <Button className="shared-dashboard-workspace" key={organization.id} onClick={() => organizationNavigation.selectOrganization(organization.id)} type="button" variant="secondary"><span className="shared-workspace-icon"><Building size={ICON_SIZE.md} /></span><span><strong>{organization.name}</strong><small>{organization.role}</small></span><span className="shared-dashboard-workspace-metrics"><span>{count} {count === 1 ? "workspace" : "workspaces"}</span></span><ChevronRight size={ICON_SIZE.md} /></Button>;
      })}</div> : <div className="shared-empty-state"><Building size={26} /><strong>No organizations yet</strong>{canCreate ? <><span>Create an organization to start a shared workspace boundary.</span>{create}</> : <span>Ask a system administrator to create one or to invite you to one.</span>}</div>}
    </section>
  </SharedLayout>;
}
