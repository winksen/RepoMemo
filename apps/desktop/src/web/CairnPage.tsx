import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import { useEffect, useState } from "react";
import { CaretDown as ChevronDown, Plus, Stack as Layers } from "@phosphor-icons/react";
import { AssistantPanel } from "../components/AssistantPanel";
import { CairnIcon } from "../components/CairnIcon";
import { SharedLayout } from "../components/SharedLayout";
import { Button } from "../components/ui/button";
import { IconButton } from "../components/ui/icon-button";
import { Tooltip } from "../components/ui/tooltip";
import { EmptyState } from "../components/ui/empty-state";
import { ICON_SIZE } from "../components/ui/icon";
import { PageHeader } from "../components/ui/page-header";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { workspaceKeys } from "../lib/queryClient";
import { getSharedAgentCapabilities } from "../lib/sharedApi";
import { useServerState } from "../lib/useServerState";
import { openNewWorkspace } from "../lib/newWorkspaceBridge";
import { type AgentCapabilities, type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { formatRecentTime } from "./format";
import { RailSection } from "./Organization";
import { type CairnChat, cairnContextWorkspace, groupCairnChats, onCairnWorkspaceChange, rememberCairnWorkspace, rememberedCairnWorkspace, useCairnChats } from "./cairn";
import { cairnPath, navigate, routePath } from "./router";
import { WorkspaceIcon } from "./workspaceIcons";

/** One workspace's chats in the sidebar: its name folds them away, and its + starts a chat in it. */
function CairnGroup({ activeId, chats, workspace }: { activeId: string | null; chats: CairnChat[]; workspace: SharedWorkspace }) {
  const [open, setOpen] = useState(true);
  const [all, setAll] = useState(false);
  const visible = all || chats.length <= GROUP_LIMIT ? chats : chats.slice(0, GROUP_LIMIT);
  return <section aria-label={workspace.workspace.name} className="shared-cairn-group">
    <div className="shared-cairn-group-head">
      <button aria-expanded={open} className="shared-cairn-group-toggle" onClick={() => setOpen((current) => !current)} type="button"><span className="shared-cairn-group-icon"><WorkspaceIcon size={14} weight="fill" workspace={workspace.workspace} /></span><span className="shared-cairn-group-name">{workspace.workspace.name}</span><ChevronDown size={ICON_SIZE.sm} /></button>
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

/** The workspace a chat draws on, above the message box, with its own icon: a menu for a new chat, a plain label once the chat has begun. */
function WorkspaceChooser({ editable, onChoose, organizations, selected, workspaces }: { editable: boolean; onChoose: (workspaceId: string) => void; organizations: Organization[]; selected: SharedWorkspace; workspaces: SharedWorkspace[] }) {
  const face = (entry: SharedWorkspace) => <><span className="shared-cairn-workspace-icon"><WorkspaceIcon size={16} weight="fill" workspace={entry.workspace} /></span><span className="shared-cairn-workspace-name">{workspaceLabel(entry, organizations)}</span></>;
  if (!editable) return <div className="shared-cairn-workspace" title="The workspace this chat draws on">{face(selected)}</div>;
  return <MenuPrimitive.Root>
    <MenuPrimitive.Trigger asChild>
      <button aria-label="Workspace for this chat" className="shared-cairn-workspace shared-cairn-workspace-button" type="button">{face(selected)}<ChevronDown size={ICON_SIZE.sm} /></button>
    </MenuPrimitive.Trigger>
    <MenuPrimitive.Portal>
      <MenuPrimitive.Content align="start" aria-label="Workspace for this chat" className="rm-action-menu" side="top" sideOffset={6}>
        {workspaces.map((entry) => <MenuPrimitive.Item className="rm-action-item" key={entry.workspace.id} onSelect={() => onChoose(entry.workspace.id)}>
          <WorkspaceIcon size={ICON_SIZE.md} weight={entry.workspace.id === selected.workspace.id ? "fill" : "regular"} workspace={entry.workspace} /><span>{workspaceLabel(entry, organizations)}</span>
        </MenuPrimitive.Item>)}
      </MenuPrimitive.Content>
    </MenuPrimitive.Portal>
  </MenuPrimitive.Root>;
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
  // The heading is the chat's title (in the page-title face); a new chat has none yet. The capabilities are the panel's too (the same cache).
  const [optionsHost, setOptionsHost] = useState<HTMLElement | null>(null);
  const [capabilities] = useServerState<AgentCapabilities | null>({
    key: workspaceKeys.assistantCapabilities(chosenId ?? ""), fetch: () => getSharedAgentCapabilities(accessToken, chosenId ?? ""), initial: null, enabled: Boolean(chosenId),
  });
  const chatTitle = open ? open.conversation.title : chatId ? "Chat" : "New chat";

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
        actions={<span className="shared-chat-options-host" ref={setOptionsHost} />}
        icon={<CairnIcon size={32} weight="fill" />}
        title={chatTitle}
      />
      {chosen
        ? <div className="shared-assistant-page-body"><AssistantPanel accessToken={accessToken} composerHeader={<WorkspaceChooser editable={isNewChat} onChoose={rememberCairnWorkspace} organizations={organizations} selected={chosen} workspaces={workspaces} />} hideChatList key={chosenId} optionsHost={optionsHost} onChatChange={(id) => navigate(cairnPath(id), true)} onOpenArtifact={(artifactId) => navigate(routePath({ name: "artifact", workspaceId: chosen.workspace.id, artifactId }))} requestedChatId={chatId} variant="page" workspaceId={chosen.workspace.id} /></div>
        : <EmptyState action={<Button onClick={() => openNewWorkspace()} type="button" variant="main">New workspace</Button>} description={`${ASSISTANT_NAME} works on the evidence of a workspace. Create one, or ask an administrator to add you to one.`} title="No workspace to talk about yet" />}
    </section>
  </SharedLayout>;
}
