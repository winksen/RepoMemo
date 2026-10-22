import { useEffect } from "react";
import { LoadingCanvas } from "./common";
import { navigate, routePath } from "./router";
import { landingSection } from "./templateSections";
import { useTemplate } from "./workspace/data";

/**
 * What `/workspaces/<id>` shows: it asks for the workspace's template and replaces its own address with the
 * landing section, so the back button leaves the workspace instead of coming back here. If the template cannot
 * be read the workspace opens on Overview, as it always did.
 */
export function WorkspaceLanding({ accessToken, workspaceId }: { accessToken: string; workspaceId: string }) {
  const [template, , query] = useTemplate(accessToken, workspaceId);
  const settled = template !== null || query.isError;
  const section = landingSection(template?.sections);
  useEffect(() => {
    if (settled) navigate(routePath({ name: "workspace", workspaceId, section }), true);
  }, [section, settled, workspaceId]);
  return <LoadingCanvas label="Opening the workspace" />;
}
