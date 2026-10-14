import { SharedLayout } from "../components/SharedLayout";
import { SystemAdminPanel, SystemNavigation } from "../components/SystemAdminPanel";
import type { Organization, SharedSession, SharedWorkspace } from "../types";
import type { SystemSection } from "./constants";
import { OrganizationRail } from "./Organization";
import { navigate } from "./router";

/** The System area. Loaded on demand: most people never open it. */
export default function SystemArea({
  accessToken,
  apiAvailable,
  organizations,
  section,
  session,
  signOut,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  organizations: Organization[];
  section: SystemSection;
  session: SharedSession;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  return (
    <SharedLayout
      apiAvailable={apiAvailable}
      onNavigate={navigate}
      session={session}
      signOut={signOut}
      sidebar={<OrganizationRail organizations={organizations} showSelection={false} workspaces={workspaces} />}
      workspaceNavigation={<SystemNavigation active={section} onNavigate={(next) => navigate(`/system/${next}`)} />}
    >
      <SystemAdminPanel
        accessToken={accessToken}
        key={section}
        onNavigateSection={(next) => navigate(`/system/${next}`)}
        onOpenWorkspace={(id) => navigate(`/workspaces/${encodeURIComponent(id)}/overview`)}
        section={section}
        session={session}
        workspaces={workspaces}
      />
    </SharedLayout>
  );
}
