import { useEffect } from "react";
import { IconMessage as Message, IconPlus as Plus, IconStack2 as Layers } from "@tabler/icons-react";
import { AssistantPanel } from "../components/AssistantPanel";
import { CairnIcon } from "../components/CairnIcon";
import { SharedLayout } from "../components/SharedLayout";
import { Button } from "../components/ui/button";
import { EmptyState } from "../components/ui/empty-state";
import { ICON_SIZE } from "../components/ui/icon";
import { PageHeader } from "../components/ui/page-header";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { openNewWorkspace } from "../lib/newWorkspaceBridge";
import { type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { formatRecentTime } from "./format";
import { RailList, RailSection } from "./Organization";
import { cairnContextWorkspace, rememberCairnWorkspace, rememberedCairnWorkspace, useCairnChats } from "./cairn";
import { cairnPath, navigate, routePath } from "./router";
import { workspaceColorKey, workspaceColorProps } from "./workspaceColors";

/** The sidebar of Cairn's page: the person's chats, across every workspace, with *New chat* above them. */
export function CairnRail({ accessToken, chatId, organizations, workspaces }: { accessToken: string; chatId: string | null; organizations: Organization[]; workspaces: SharedWorkspace[] }) {
  const { chats, loaded } = useCairnChats(accessToken, workspaces);
  const items = chats.map(({ conversation, workspace }) => {
    const organization = organizations.find((entry) => entry.id === workspace.organization_id);
    return {
      id: conversation.id,
      name: conversation.title,
      title: `${conversation.title} · ${organization && organizations.length > 1 ? `${organization.name} / ` : ""}${workspace.workspace.name}`,
      subtitle: <><Layers size={12} /><span>{workspace.workspace.name} · {formatRecentTime(conversation.updated_at)}</span></>,
      colorProps: workspaceColorProps(workspaceColorKey(workspace.workspace)),
      icon: <span className="shared-rail-chip"><Message size={22} /></span>,
      onSelect: () => navigate(cairnPath(conversation.id)),
    };
  });
  return <div className="shared-rail-navigation">
    <RailSection label="Chats" name="chats">
      <Button className="shared-rail-new-chat" onClick={() => navigate(cairnPath(null))} type="button" variant="secondary"><Plus size={ICON_SIZE.md} /> New chat</Button>
      {items.length ? <RailList activeId={chatId ?? undefined} items={items} /> : <p className="shared-rail-empty">{loaded ? "Your chats appear here. Only you can see them." : "Loading chats…"}</p>}
    </RailSection>
  </div>;
}

/**
 * Cairn on a whole page: a chat with the chat history in the sidebar. Cairn is one assistant for the person, so there is no
 * workspace to choose: a chat that exists belongs to the workspace it was started in, and a new one starts in the workspace the
 * person came from (or worked in last). Until the server answers across workspaces, that workspace is what Cairn draws on.
 */
export function SharedCairnPage({ accessToken, chatId, session, signOut, workspaces }: {
  accessToken: string;
  chatId: string | null;
  session: SharedSession;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  const { chats, loaded } = useCairnChats(accessToken, workspaces);
  const open = chatId ? chats.find((chat) => chat.conversation.id === chatId) : undefined;
  const chosen = open?.workspace ?? cairnContextWorkspace(workspaces, rememberedCairnWorkspace());
  const chosenId = chosen?.workspace.id ?? null;

  // A chat that no longer exists (deleted, or not the person's) opens a new one.
  useEffect(() => {
    if (chatId && loaded && !open) navigate(cairnPath(null), true);
  }, [chatId, loaded, open]);
  // The workspace of the chat that is open is where the next new chat starts.
  useEffect(() => { if (open) rememberCairnWorkspace(open.workspace.workspace.id); }, [open?.workspace.workspace.id]);

  return <SharedLayout
    accent={chosen ? workspaceColorKey(chosen.workspace) : null}
    onNavigate={navigate}
    session={session}
    sidebar={null}
    signOut={signOut}
  >
    <section className="shared-page-content shared-assistant-page">
      <PageHeader
        actions={chosen ? <span className="shared-cairn-context" title="The workspace Cairn draws on in this chat"><Layers size={ICON_SIZE.sm} /> {chosen.workspace.name}</span> : null}
        description="Ask a question, search the evidence or get a workspace summarized, in conversation."
        icon={<CairnIcon size={ICON_SIZE.lg} />}
        title={ASSISTANT_NAME}
      />
      {chosen
        ? <div className="shared-assistant-page-body"><AssistantPanel accessToken={accessToken} hideChatList key={chosenId} onChatChange={(id) => navigate(cairnPath(id), true)} onOpenArtifact={(artifactId) => navigate(routePath({ name: "artifact", workspaceId: chosen.workspace.id, artifactId }))} requestedChatId={chatId} workspaceId={chosen.workspace.id} /></div>
        : <EmptyState action={<Button onClick={() => openNewWorkspace()} type="button" variant="main">New workspace</Button>} description={`${ASSISTANT_NAME} works on the evidence of a workspace. Create one, or ask an administrator to add you to one.`} title="No workspace to talk about yet" />}
    </section>
  </SharedLayout>;
}
