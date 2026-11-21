import { IconBuildingCommunity as Building, IconChevronRight as ChevronRight, IconPlus as Plus } from "@tabler/icons-react";
import { SharedLayout } from "../components/SharedLayout";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { ICON_SIZE } from "../components/ui/icon";
import { PageHeader } from "../components/ui/page-header";
import { type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { useOrganizationNavigation } from "./navigation";
import { navigate } from "./router";
import { OrganizationRail } from "./Organization";
import { WorkspaceIcon } from "./workspaceIcons";

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
    <section className="shared-page-content shared-orgs-page">
      <PageHeader actions={organizations.length ? create : null} description="Every organization you belong to. Open one to manage its members, templates and workspaces." title="Organizations" />
      {organizations.length ? <div className="shared-org-cards">{organizations.map((organization) => {
        const own = workspaces.filter((workspace) => workspace.organization_id === organization.id);
        return <Button className="shared-org-card" key={organization.id} onClick={() => organizationNavigation.selectOrganization(organization.id)} type="button" variant="secondary">
          <span className="shared-org-card-head"><span className="shared-org-card-mark"><Building size={26} /></span><Badge>{organization.role}</Badge></span>
          <strong>{organization.name}</strong>
          <span className="shared-org-card-count">{own.length} {own.length === 1 ? "workspace" : "workspaces"}</span>
          <span className="shared-org-card-spaces">{own.slice(0, 4).map((workspace) => <span key={workspace.workspace.id}><WorkspaceIcon size={ICON_SIZE.sm} workspace={workspace.workspace} />{workspace.workspace.name}</span>)}{own.length > 4 ? <span>+{own.length - 4} more</span> : null}{own.length === 0 ? <span>No workspaces yet</span> : null}</span>
          <ChevronRight className="shared-org-card-arrow" size={ICON_SIZE.lg} />
        </Button>;
      })}</div> : <div className="shared-empty-state"><Building size={26} /><strong>No organizations yet</strong>{canCreate ? <><span>Create an organization to start a shared workspace boundary.</span>{create}</> : <span>Ask a system administrator to create one or to invite you to one.</span>}</div>}
    </section>
  </SharedLayout>;
}
