import { useEffect } from "react";
import { navigate, routePath } from "./router";

/**
 * What `/spaces/<id>` shows: nothing. It replaces its own address with the workspace's Overview, so the back button
 * leaves the workspace instead of coming back here. A workspace always opens on Overview, whatever its template says
 * (V0.8.31); the template still orders the tabs.
 */
export function WorkspaceLanding({ workspaceId }: { workspaceId: string }) {
  useEffect(() => { navigate(routePath({ name: "workspace", workspaceId, section: "overview" }), true); }, [workspaceId]);
  return null;
}
