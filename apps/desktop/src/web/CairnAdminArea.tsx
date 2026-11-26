import { useEffect } from "react";
import { SharedLayout } from "../components/SharedLayout";
import { EmptyState } from "../components/ui/empty-state";
import type { SharedSession } from "../types";
import { CAIRN_ADMIN_SECTIONS, type CairnAdminSection } from "./constants";
import { allowedSections, CairnAdminLoading, CairnAdminPanel, useCairnAdminView } from "./cairn-admin/CairnAdminPanel";
import { cairnAdminPath, navigate } from "./router";

/** The CAIRN administration area. Loaded on demand: most people never open it. */
export default function CairnAdminArea({
  accessToken,
  apiAvailable,
  section,
  session,
  signOut,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  section: CairnAdminSection;
  session: SharedSession;
  signOut: () => void;
}) {
  const [view, isLoading, reload, , error] = useCairnAdminView(accessToken);
  const sections = allowedSections(view);
  // A section the person may not open leads to the first one they may.
  const shown: CairnAdminSection = sections.includes(section) ? section : (sections[0] ?? CAIRN_ADMIN_SECTIONS[0]);
  useEffect(() => {
    if (view && shown !== section) navigate(cairnAdminPath(shown), true);
  }, [view, shown, section]);
  return (
    <SharedLayout
      apiAvailable={apiAvailable}
      onNavigate={navigate}
      session={session}
      signOut={signOut}
      sidebar={null}
    >
      {view ? <CairnAdminPanel isLoading={isLoading} key={shown} onReload={() => void reload()} section={shown} view={view} /> : error ? <section className="shared-page-content rm-system-page"><EmptyState description="Only CAIRN, system and app administrators, and owners and admins of an organization, can open the CAIRN administration." title="You cannot open the CAIRN administration" /></section> : <CairnAdminLoading />}
    </SharedLayout>
  );
}
