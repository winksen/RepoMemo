import { useEffect, useState } from "react";
import { CaretDown as ChevronDown, Plus, Stack as Layers } from "@phosphor-icons/react";
import { AssistantPanel } from "../components/AssistantPanel";
import { CairnIcon } from "../components/CairnIcon";
import { SharedLayout } from "../components/SharedLayout";
import { Button } from "../components/ui/button";
import { Dropdown } from "../components/ui/dropdown";
import { IconButton } from "../components/ui/icon-button";
import { Tooltip } from "../components/ui/tooltip";
import { EmptyState } from "../components/ui/empty-state";
import { ICON_SIZE } from "../components/ui/icon";
import { PageHeader } from "../components/ui/page-header";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { openNewWorkspace } from "../lib/newWorkspaceBridge";
import { type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { formatRecentTime } from "./format";
import { RailSection } from "./Organization";
import { type CairnChat, cairnContextWorkspace, groupCairnChats, onCairnWorkspaceChange, rememberCairnWorkspace, rememberedCairnWorkspace, useCairnChats } from "./cairn";
import { cairnPath, navigate, routePath } from "./router";

/** One workspace's chats in the sidebar: its name folds them away, and its + starts a chat in it. */
function CairnGroup({ activeId, chats, workspace }: { activeId: string | null; chats: CairnChat[]; workspace: SharedWorkspace }) {
  const [open, setOpen] = useState(true);
  const [all, setAll] = useState(false);
  const visible = all || chats.length <= GROUP_LIMIT ? chats : chats.slice(0, GROUP_LIMIT);
  return <section aria-label={workspace.workspace.name} className="shared-cairn-group">
    <div className="shared-cairn-group-head">
      <button aria-expanded={open} className="shared-cairn-group-toggle" onClick={() => setOpen((current) => !current)} type="button"><span>{workspace.workspace.name}</span><ChevronDown size={ICON_SIZE.sm} /></button>
      <IconButton appearance="plain" className="shared-cairn-group-new" label={`New chat in ${workspace.workspace.name}`} onClick={() => { rememberCairnWorkspace(workspace.workspace.id); navigate(cairnPath(null)); }} size="sm"><Plus size={ICON_SIZE.sm} /></IconButton>
    </div>
    <div hidden={!open}>
      {visible.map(({ conversation }) => <Tooltip content={`${conversation.title} · ${formatRecentTime(conversation.updated_at)}`} key={conversation.id} side="right">
        <Button aria-current={conversation.id === activeId ? "page" : undefined} className={`shared-cairn-chat${conversation.id === activeId ? " active" : ""}`} onClick={() => navigate(cairnPath(conversation.id))} type="button" variant="secondary"><span>{conversation.title}</span></Button>
      </Tooltip>)}
      {chats.length > GROUP_LIMIT ? <Button className="shared-rail-more" onClick={() => setAll((current) => !current)} type="button" variant="secondary">{all ? "Show less" : `Show more (${chats.length - GROUP_LIMIT})`}</Button> : null}
    </div>
  </section>;
}

const GROUP_LIMIT = 6;

/** The sidebar of Cairn's page: the person's chats, grouped by the workspace each belongs to, with *New chat* above them. Nothing else. */
export function CairnRail({ accessToken, chatId, workspaces }: { accessToken: string; chatId: string | null; workspaces: SharedWorkspace[] }) {
  const { chats, loaded } = useCairnChats(accessToken, workspaces);
  const groups = groupCairnChats(chats);
  return <div className="shared-rail-navigation">
    <RailSection label="Chats" name="chats">
      <Button className="shared-rail-new-chat" onClick={() => navigate(cairnPath(null))} type="button" variant="secondary"><Plus size={ICON_SIZE.md} /> New chat</Button>
      {groups.length ? groups.map((group) => <CairnGroup activeId={chatId} chats={group.chats} key={group.workspace.workspace.id} workspace={group.workspace} />) : <p className="shared-rail-empty">{loaded ? "Your chats appear here. Only you can see them." : "Loading chats…"}</p>}
    </RailSection>
  </div>;
}

/** A workspace as the chooser and the label name it: with its organization when the person has several. */
function workspaceLabel(entry: SharedWorkspace, organizations: Organization[]): string {
  const organization = organizations.find((candidate) => candidate.id === entry.organization_id);
  return organization && organizations.length > 1 ? `${organization.name} / ${entry.workspace.name}` : entry.workspace.name;
}

/**
 * Cairn on a whole page: a chat with the chat history in the sidebar, grouped by workspace. A chat belongs to the workspace it was
 * started in (shown as a label); a new chat offers a choice of workspace, starting from the one the person came from or worked in
 * last. Until the server answers across workspaces, that workspace is what Cairn draws on.
 */
export function SharedCairnPage({ accessToken, chatId, organizations, session, signOut, workspaces }: {
  accessToken: string;
  chatId: string | null;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  const { chats, loaded } = useCairnChats(accessToken, workspaces);
  const open = chatId ? chats.find((chat) => chat.conversation.id === chatId) : undefined;
  // For a new chat the person chooses the workspace; the choice is kept for the next one.
  const [startIn, setStartIn] = useState(rememberedCairnWorkspace);
  useEffect(() => onCairnWorkspaceChange(() => setStartIn(rememberedCairnWorkspace())), []);
  const chosen = open?.workspace ?? cairnContextWorkspace(workspaces, startIn);
  const isNewChat = chatId === null;
  const chosenId = chosen?.workspace.id ?? null;

  // A chat that no longer exists (deleted, or not the person's) opens a new one.
  useEffect(() => {
    if (chatId && loaded && !open) navigate(cairnPath(null), true);
  }, [chatId, loaded, open]);
  // The workspace of the chat that is open is where the next new chat starts.
  useEffect(() => { if (open) rememberCairnWorkspace(open.workspace.workspace.id); }, [open?.workspace.workspace.id]);

  return <SharedLayout
    onNavigate={navigate}
    session={session}
    sidebar={null}
    signOut={signOut}
  >
    <section className="shared-page-content shared-assistant-page">
      <PageHeader
        actions={chosen
          ? isNewChat
            ? <Dropdown aria-label="Workspace for this chat" className="shared-assistant-page-workspace" onValueChange={rememberCairnWorkspace} options={workspaces.map((entry) => ({ label: workspaceLabel(entry, organizations), value: entry.workspace.id }))} value={chosen.workspace.id} />
            : <span className="shared-cairn-context" title="The workspace this chat draws on"><Layers size={ICON_SIZE.sm} /> {workspaceLabel(chosen, organizations)}</span>
          : null}
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
