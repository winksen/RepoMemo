import { useEffect } from "react";
import { IconSparkles as Sparkles } from "@tabler/icons-react";
import { AssistantPanel } from "../components/AssistantPanel";
import { SharedLayout } from "../components/SharedLayout";
import { Button } from "../components/ui/button";
import { Dropdown } from "../components/ui/dropdown";
import { EmptyState } from "../components/ui/empty-state";
import { ICON_SIZE } from "../components/ui/icon";
import { PageHeader } from "../components/ui/page-header";
import { openNewWorkspace } from "../lib/newWorkspaceBridge";
import { type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { OrganizationRail } from "./Organization";
import { assistantPath, navigate, routePath } from "./router";
import { workspaceColorKey } from "./workspaceColors";

const LAST_WORKSPACE_KEY = "repomemo.assistant.page-workspace";

function rememberedWorkspace(): string | null {
  try { return window.localStorage.getItem(LAST_WORKSPACE_KEY); } catch { return null; }
}

/**
 * Which workspace the assistant page talks about: the one in the address when the person can open it, else the one
 * used last time, else the first. Nothing when there is none.
 */
export function assistantWorkspace(workspaces: SharedWorkspace[], requested: string | null, remembered: string | null): SharedWorkspace | undefined {
  const named = (id: string | null) => id ? workspaces.find((entry) => entry.workspace.id === id) : undefined;
  return named(requested) ?? named(remembered) ?? workspaces[0];
}

/**
 * The assistant on a whole page (V0.8.46): the chat takes the page, with the workspace it works on chosen above it.
 * The dock on a workspace page is the same assistant, beside the work; this is the place to start from.
 */
export function SharedAssistantPage({ accessToken, organizations, requestedWorkspaceId, session, signOut, workspaces }: {
  accessToken: string;
  organizations: Organization[];
  requestedWorkspaceId: string | null;
  session: SharedSession;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  const chosen = assistantWorkspace(workspaces, requestedWorkspaceId, rememberedWorkspace());
  const chosenId = chosen?.workspace.id ?? null;

  // The address names the workspace, so a reload or a shared link comes back to the same one.
  useEffect(() => {
    if (!chosenId) return;
    try { window.localStorage.setItem(LAST_WORKSPACE_KEY, chosenId); } catch { /* only a convenience */ }
    if (requestedWorkspaceId !== chosenId) navigate(assistantPath(chosenId), true);
  }, [chosenId, requestedWorkspaceId]);

  const options = workspaces.map((entry) => {
    const organization = organizations.find((candidate) => candidate.id === entry.organization_id);
    return { label: organization && organizations.length > 1 ? `${organization.name} / ${entry.workspace.name}` : entry.workspace.name, value: entry.workspace.id };
  });

  return <SharedLayout
    accent={chosen ? workspaceColorKey(chosen.workspace) : null}
    onNavigate={navigate}
    session={session}
    sidebar={<OrganizationRail organizations={organizations} workspaceId={chosenId ?? undefined} workspaces={workspaces} />}
    signOut={signOut}
  >
    <section className="shared-page-content shared-assistant-page">
      <PageHeader
        actions={chosen ? <Dropdown aria-label="Workspace the assistant works on" className="shared-assistant-page-workspace" onValueChange={(value) => navigate(assistantPath(value))} options={options} value={chosenId ?? ""} /> : null}
        description="Ask questions, search the evidence and get your workspace summarized in conversation."
        icon={<Sparkles size={ICON_SIZE.lg} />}
        title="Assistant"
      />
      {chosen
        ? <div className="shared-assistant-page-body"><AssistantPanel accessToken={accessToken} key={chosenId} onOpenArtifact={(artifactId) => navigate(routePath({ name: "artifact", workspaceId: chosen.workspace.id, artifactId }))} workspaceId={chosen.workspace.id} /></div>
        : <EmptyState action={<Button onClick={() => openNewWorkspace()} type="button" variant="main">New workspace</Button>} description="The assistant works on the evidence of a workspace. Create one, or ask an administrator to add you to one." title="No workspace to talk about yet" />}
    </section>
  </SharedLayout>;
}
