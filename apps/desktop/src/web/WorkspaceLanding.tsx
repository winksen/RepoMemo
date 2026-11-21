import { useEffect } from "react";
import { IconLoader2 as Loader } from "@tabler/icons-react";
import { ShellContent } from "../components/SharedLayout";
import { ICON_SIZE } from "../components/ui/icon";
import { navigate, routePath } from "./router";
import { landingSection } from "./templateSections";
import { useTemplate } from "./workspace/data";

/**
 * What `/spaces/<id>` shows: it asks for the workspace's template and replaces its own address with the
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
  // Drawn inside the frame that is already on screen, so opening a workspace does not replace the whole page.
  return <ShellContent><div className="shared-loading" role="status"><Loader className="spin" size={ICON_SIZE.lg} /> Opening the workspace</div></ShellContent>;
}
