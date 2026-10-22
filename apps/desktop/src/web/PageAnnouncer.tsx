import { RouteAnnouncer } from "./RouteAnnouncer";
import { pageKey, pageTitle } from "./pageTitle";
import type { Route } from "./router";
import { useTemplate } from "./workspace/data";

/** The announcement of the page (V0.2.173): the tab title and what a screen reader hears after a move, with the workspace's own names for its sections. */
export function PageAnnouncer({ accessToken, route, workspaceName }: { accessToken: string; route: Route; workspaceName?: string }) {
  const workspaceId = route.name === "workspace" ? route.workspaceId : null;
  const [template] = useTemplate(accessToken, workspaceId ?? "", Boolean(workspaceId));
  return <RouteAnnouncer pageKey={pageKey(route)} title={pageTitle(route, { workspace: workspaceName, sectionLabels: template?.labels.sections })} />;
}
