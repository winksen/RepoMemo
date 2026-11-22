import type { FormEvent, KeyboardEvent, ReactNode } from "react";
import { SafeMarkdown } from "./SafeMarkdown";
import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { BookmarkSimple, Brain, FolderSimple, Paperclip, FileMagnifyingGlass as FileSearch, FileText, ClockCounterClockwise as History, SquaresFour as Dashboard, Chat as Message, Question, PencilSimple as Pencil, Plus, ArrowsClockwise as Refresh, MagnifyingGlass as Search, PaperPlaneTilt as Send, Trash, X as Clear, ArrowElbowDownLeft } from "@phosphor-icons/react";
import { attachSharedAgentFile, confirmSharedAgentProposal, deleteSharedAgentConversation, dismissSharedAgentProposal, getSharedAgentCapabilities, getSharedAgentConversation, listSharedAgentConversations, renameSharedAgentConversation, sendSharedAgentMessage, uploadSharedArtifact } from "../lib/sharedApi";
import type { AgentCapabilities, AgentCapability, AgentConversation, AgentMessage, AgentProposal, AgentReply, AgentTurn } from "../types";
import { ViewAtCommit } from "../web/common";
import { useTemplate } from "../web/workspace/data";
import { orderCapabilities, suggestedQuestions } from "../web/templateAssistant";
import { CairnIcon } from "./CairnIcon";
import { ChatOptions } from "./ChatOptions";
import { ActionMenu } from "./ui/action-menu";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Field } from "./ui/field";
import { IconButton } from "./ui/icon-button";
import { Input } from "./ui/input";
import { LoadingState } from "./ui/loading-state";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage, citationChange, formatRecentTime } from "../web/format";
import { workspaceKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import { useErrorToast } from "../lib/useErrorToast";
import { useAssistantCommands } from "../lib/useAssistantCommands";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { submitShortcutLabel } from "../lib/submitShortcut";

/** A turn as shown: stored turns plus the one waiting for, or failing to get, a reply. */
type ThreadTurn = {
  key: string;
  label: string;
  request: AgentMessage;
  status: "pending" | "done" | "failed";
  reply?: AgentReply;
};

type ChatDialog = { kind: "rename" | "delete"; conversation: AgentConversation };

const CAPABILITY_ICONS: Record<AgentCapability, typeof Search> = {
  find_files: FileSearch,
  find_symbol: FileSearch,
  search_content: Search,
  summarize_file: FileText,
  ask_question: Question,
  workspace_overview: Dashboard,
  save_memory: BookmarkSimple,
  manage_files: FolderSimple,
  summarize_chat: History,
  analyze_files: Brain,
};

const LAST_CHAT_PREFIX = "repomemo.assistant.last-chat.";

function rememberChat(workspaceId: string, conversationId: string | null) {
  try {
    if (conversationId) window.localStorage.setItem(LAST_CHAT_PREFIX + workspaceId, conversationId);
    else window.localStorage.removeItem(LAST_CHAT_PREFIX + workspaceId);
  } catch { /* only a convenience */ }
}

function rememberedChat(workspaceId: string): string | null {
  try {
    return window.localStorage.getItem(LAST_CHAT_PREFIX + workspaceId);
  } catch {
    return null;
  }
}

function storedTurn(turn: AgentTurn): ThreadTurn {
  return { key: turn.id, label: turn.label, request: turn.request, status: "done", reply: turn.reply };
}

/** Workspace assistant: routes each message to one capability on the server and keeps the user's chats there. */
export function AssistantPanel({ accessToken, composerHeader, hideChatList = false, onChatChange, onOpenArtifact, optionsHost, requestedChatId, variant = "dock", workspaceId }: {
  accessToken: string;
  /** On Cairn's page, where the chat's options are drawn (the page's header); without it the options are not shown. */
  optionsHost?: HTMLElement | null;
  /** On Cairn's page, what sits above the message box (the workspace the chat draws on). */
  composerHeader?: ReactNode;
  /** `page` is Cairn's page: the message box is one box with *Send* at its left and a **+** menu of actions at its right, and nothing under it. */
  variant?: "dock" | "page";
  /** Cairn's page lists the chats beside the page, so the panel does not offer its own list or *New chat*. */
  hideChatList?: boolean;
  /** Called with the chat the panel itself moved to: a chat that was just started (its id), or none after *New chat* or a deletion. */
  onChatChange?: (conversationId: string | null) => void;
  onOpenArtifact: (artifactId: string) => void;
  /** Gives the chat that is open to whoever holds the address: the panel follows it (`null` is a new chat). Left out, the panel keeps its own. */
  requestedChatId?: string | null;
  workspaceId: string;
}) {
  const [capabilities, , capabilitiesQuery] = useServerState<AgentCapabilities | null>({
    key: workspaceKeys.assistantCapabilities(workspaceId),
    fetch: () => getSharedAgentCapabilities(accessToken, workspaceId),
    initial: null,
  });
  // The person's chats of this workspace; handlers that rename, delete or add one write to the cache.
  const [conversations, setConversations, chatsQuery] = useServerState<AgentConversation[]>({
    key: workspaceKeys.assistantChats(workspaceId),
    fetch: () => listSharedAgentConversations(accessToken, workspaceId),
    initial: [],
    staleTime: 0,
  });
  useErrorToast(capabilitiesQuery.error ?? chatsQuery.error, `${ASSISTANT_NAME} is unavailable.`);
  const [template] = useTemplate(accessToken, workspaceId);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [turns, setTurns] = useState<ThreadTurn[]>([]);
  const [isLoadingChat, setIsLoadingChat] = useState(false);
  const [selected, setSelected] = useState<AgentCapability | null>(null);
  const [draft, setDraft] = useState("");
  const [chatDialog, setChatDialog] = useState<ChatDialog | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [isSavingChat, setIsSavingChat] = useState(false);
  // The proposal being confirmed or dismissed (turn and proposal ids), so its buttons wait.
  const [deciding, setDeciding] = useState<string | null>(null);
  // Files being uploaded into the chat (V0.9.12); the message box waits for them.
  const [isUploading, setIsUploading] = useState(false);
  // The files selected in each reply's list, by turn (V0.9.26), and the turn whose selection is being moved.
  const [selection, setSelection] = useState<Record<string, string[]>>({});
  const [moveFor, setMoveFor] = useState<string | null>(null);
  const [moveFolder, setMoveFolder] = useState("");
  const fileInput = useRef<HTMLInputElement>(null);
  const [showChats, setShowChats] = useState(false);
  // Bumped whenever the visible chat changes, so a reply that arrives after
  // the user moved on is not written into the wrong thread.
  const view = useRef(0);
  const pendingCounter = useRef(0);
  const threadEnd = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const isBusy = turns.some((turn) => turn.status === "pending") || isUploading;
  // Whoever may change the workspace may upload into it; the server says so through the jobs it offers.
  const canUpload = capabilities?.capabilities.some((capability) => capability.id === "manage_files" && capability.available) ?? false;
  const selectedInfo = capabilities?.capabilities.find((capability) => capability.id === selected) ?? null;
  const canSummarize = capabilities?.capabilities.some((capability) => capability.id === "summarize_file" && capability.available) ?? false;
  const activeConversation = conversations.find((conversation) => conversation.id === activeId) ?? null;

  // Once the chats are known, open the one this person had open last time.
  const restoredFor = useRef<string | null>(null);
  const chatsLoaded = chatsQuery.data !== undefined;
  useEffect(() => {
    if (!chatsLoaded || restoredFor.current === workspaceId || requestedChatId !== undefined) return;
    restoredFor.current = workspaceId;
    const last = rememberedChat(workspaceId);
    if (last && conversations.some((conversation) => conversation.id === last)) void openChat(last);
  }, [chatsLoaded, workspaceId]);
  // The message box on Cairn's page is as tall as its text, up to a limit; it scrolls beyond that.
  useEffect(() => {
    const field = input.current;
    if (!field || variant !== "page") return;
    field.style.height = "auto";
    field.style.height = `${Math.min(field.scrollHeight, 220)}px`;
  }, [draft, variant]);
  // On Cairn's page the address decides which chat is open.
  useEffect(() => {
    if (requestedChatId === undefined || requestedChatId === activeId) return;
    if (requestedChatId) void openChat(requestedChatId);
    else startNewChat();
  }, [requestedChatId]);
  useEffect(() => { threadEnd.current?.scrollIntoView({ block: "end", behavior: "smooth" }); }, [turns.length, isBusy]);

  // Commands from the command palette wait until the capabilities are loaded and the chat is idle.
  useAssistantCommands({
    ready: Boolean(capabilities) && !isBusy && !isLoadingChat,
    handle: (queued) => {
      const label = queued.message.trim() || (queued.capability ? capabilityLabel(capabilities, queued.capability) : "");
      if (queued.capability === "workspace_overview" || queued.capability === "summarize_chat" || (queued.message.trim() && label)) {
        send({ message: queued.message.trim(), capability: queued.capability }, label);
        setSelected(null);
      } else if (queued.capability) {
        // Only the action was chosen: select it and let the person type.
        setSelected(queued.capability);
        input.current?.focus();
      }
    },
  });

  async function openChat(conversationId: string) {
    const token = ++view.current;
    setActiveId(conversationId);
    rememberChat(workspaceId, conversationId);
    setTurns([]);
    setIsLoadingChat(true);
    try {
      const detail = await getSharedAgentConversation(accessToken, conversationId);
      if (token !== view.current) return;
      setTurns(detail.turns.map(storedTurn));
      upsertConversation(detail.conversation);
    } catch (error) {
      if (token !== view.current) return;
      showToast("error", apiMessage(error, "This chat could not be loaded."));
      startNewChat();
    } finally {
      if (token === view.current) setIsLoadingChat(false);
    }
  }

  function startNewChat() {
    view.current += 1;
    setActiveId(null);
    rememberChat(workspaceId, null);
    setTurns([]);
    setIsLoadingChat(false);
    input.current?.focus();
    onChatChange?.(null);
  }

  function upsertConversation(conversation: AgentConversation) {
    setConversations((current) => [conversation, ...current.filter((entry) => entry.id !== conversation.id)]
      .sort((a, b) => b.updated_at.localeCompare(a.updated_at)));
  }

  async function run(turn: ThreadTurn) {
    const token = view.current;
    const conversationId = activeId;
    setTurns((current) => [...current.filter((entry) => entry.key !== turn.key), { ...turn, status: "pending", reply: undefined }]);
    try {
      const { conversation, turn: saved } = await sendSharedAgentMessage(accessToken, workspaceId, turn.request, conversationId ?? undefined);
      upsertConversation(conversation);
      if (token !== view.current) return;
      if (!conversationId) {
        setActiveId(conversation.id);
        rememberChat(workspaceId, conversation.id);
        onChatChange?.(conversation.id);
      }
      setTurns((current) => current.map((entry) => entry.key === turn.key ? storedTurn(saved) : entry));
    } catch (error) {
      showToast("error", apiMessage(error, `${ASSISTANT_NAME} could not reply.`));
      if (token !== view.current) return;
      setTurns((current) => current.map((entry) => entry.key === turn.key ? { ...entry, status: "failed" } : entry));
    }
  }

  function send(request: AgentMessage, label: string) {
    if (isBusy || isLoadingChat) return;
    pendingCounter.current += 1;
    void run({ key: `pending-${pendingCounter.current}`, request, label, status: "pending" });
  }

  function submit(event?: FormEvent<HTMLFormElement>) {
    event?.preventDefault();
    const message = draft.trim();
    if (!message) return;
    send({ message, capability: selected }, message);
    setDraft("");
  }

  function onComposerKey(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submit();
    }
  }

  function choose(capability: AgentCapability) {
    if (capability === "workspace_overview" || capability === "summarize_chat") {
      send({ message: "", capability }, capabilityLabel(capabilities, capability));
      setSelected(null);
      return;
    }
    setSelected((current) => current === capability ? null : capability);
    input.current?.focus();
  }

  /** Uploads files into the workspace from the chat: each is stored and indexed by the upload, and CAIRN records it as a message. */
  async function uploadFiles(files: File[]) {
    if (!files.length || isBusy || isLoadingChat) return;
    const token = view.current;
    setIsUploading(true);
    let conversationId = activeId;
    try {
      for (const file of files) {
        try {
          const artifact = await uploadSharedArtifact(accessToken, workspaceId, file);
          const { conversation, turn: saved } = await attachSharedAgentFile(accessToken, workspaceId, artifact.id, conversationId ?? undefined);
          upsertConversation(conversation);
          if (token !== view.current) return;
          if (!conversationId) {
            conversationId = conversation.id;
            setActiveId(conversation.id);
            rememberChat(workspaceId, conversation.id);
            onChatChange?.(conversation.id);
          }
          setTurns((current) => [...current, storedTurn(saved)]);
        } catch (error) {
          showToast("error", apiMessage(error, `${file.name} could not be uploaded.`));
        }
      }
    } finally {
      setIsUploading(false);
    }
  }

  /** Acts on the files selected in a reply's list: the message names them by id, and CAIRN proposes the change for the person to confirm. */
  function actOnSelection(turnKey: string, action: "move" | "trash") {
    const ids = selection[turnKey] ?? [];
    if (!ids.length) return;
    if (action === "move") {
      setMoveFolder("");
      setMoveFor(turnKey);
      return;
    }
    send({ message: "Trash the selected files", capability: "manage_files", artifact_ids: ids }, `Move ${ids.length} ${ids.length === 1 ? "file" : "files"} to the trash`);
    setSelection((current) => ({ ...current, [turnKey]: [] }));
  }

  function submitMove() {
    const ids = moveFor ? selection[moveFor] ?? [] : [];
    const folder = moveFolder.trim();
    if (!moveFor || !ids.length || !folder) return;
    send({ message: `Move the selected files to ${folder}`, capability: "manage_files", artifact_ids: ids }, `Move ${ids.length} ${ids.length === 1 ? "file" : "files"} to ${folder}`);
    setSelection((current) => ({ ...current, [moveFor]: [] }));
    setMoveFor(null);
  }

  function summarize(artifactId: string, title: string) {
    send({ message: title, capability: "summarize_file", artifact_id: artifactId }, `Summarize ${title}`);
  }

  /** Confirms or dismisses what CAIRN proposed. The action runs on the server as the person; the turn comes back with its outcome. */
  async function decide(turnId: string, proposal: AgentProposal, action: "confirm" | "dismiss") {
    setDeciding(`${turnId}/${proposal.id}`);
    try {
      const saved = await (action === "confirm" ? confirmSharedAgentProposal : dismissSharedAgentProposal)(accessToken, turnId, proposal.id);
      setTurns((current) => current.map((entry) => entry.key === turnId ? storedTurn(saved) : entry));
      if (action === "confirm") showToast("success", saved.reply.proposals?.find((entry) => entry.id === proposal.id)?.outcome ?? "Done.");
    } catch (error) {
      showToast("error", apiMessage(error, action === "confirm" ? "That could not be done." : "That could not be dismissed."));
    } finally {
      setDeciding(null);
    }
  }

  async function renameChat(conversation: AgentConversation) {
    setIsSavingChat(true);
    try {
      upsertConversation(await renameSharedAgentConversation(accessToken, conversation.id, renameValue.trim()));
      setChatDialog(null);
      showToast("success", "Chat renamed.");
    } catch (error) {
      showToast("error", apiMessage(error, "The chat could not be renamed."));
    } finally {
      setIsSavingChat(false);
    }
  }

  async function deleteChat(conversation: AgentConversation) {
    setIsSavingChat(true);
    try {
      await deleteSharedAgentConversation(accessToken, conversation.id);
      setConversations((current) => current.filter((entry) => entry.id !== conversation.id));
      if (conversation.id === activeId) startNewChat();
      setChatDialog(null);
      showToast("success", "Chat deleted.");
    } catch (error) {
      showToast("error", apiMessage(error, "The chat could not be deleted."));
    } finally {
      setIsSavingChat(false);
    }
  }

  const dialog = chatDialog?.kind === "rename"
    ? <Dialog footer={<><DialogCancel onClick={() => setChatDialog(null)} /><Button busy={isSavingChat} disabled={!renameValue.trim()} type="submit" variant="main">Rename</Button></>} onClose={() => setChatDialog(null)} onSubmit={() => { if (renameValue.trim()) void renameChat(chatDialog.conversation); }} open title="Rename chat">
      <Field label="Chat name" required><Input maxLength={120} onChange={(event) => setRenameValue(event.target.value)} value={renameValue} /></Field>
    </Dialog>
    : chatDialog?.kind === "delete"
      ? <Dialog description={<>Delete <strong>{chatDialog.conversation.title}</strong> and its {chatDialog.conversation.turn_count} {chatDialog.conversation.turn_count === 1 ? "message" : "messages"}? This cannot be undone.</>} footer={<><DialogCancel onClick={() => setChatDialog(null)} /><Button busy={isSavingChat} onClick={() => void deleteChat(chatDialog.conversation)} variant="danger">Delete chat</Button></>} onClose={() => setChatDialog(null)} open title="Delete chat" />
      : null;

  const moveDialog = moveFor
    ? <Dialog description="Name the folder, or its path (Runbooks / Payments). You confirm the move before anything changes." footer={<><DialogCancel onClick={() => setMoveFor(null)} /><Button disabled={!moveFolder.trim()} type="submit" variant="main">Propose the move</Button></>} onClose={() => setMoveFor(null)} onSubmit={submitMove} open title={`Move ${(selection[moveFor] ?? []).length} ${(selection[moveFor] ?? []).length === 1 ? "file" : "files"}`}>
      <Field label="Folder" required><Input maxLength={200} onChange={(event) => setMoveFolder(event.target.value)} placeholder="Runbooks, or top level" value={moveFolder} /></Field>
    </Dialog>
    : null;

  return <section className="shared-assistant" aria-label={`${ASSISTANT_NAME}, the workspace assistant`}>
    {dialog}
    {moveDialog}
    {optionsHost ? createPortal(<ChatOptions capabilities={capabilities} conversation={activeConversation} onDelete={() => activeConversation && setChatDialog({ kind: "delete", conversation: activeConversation })} onOpenArtifact={onOpenArtifact} onRename={() => { if (activeConversation) { setRenameValue(activeConversation.title); setChatDialog({ kind: "rename", conversation: activeConversation }); } }} replies={turns.map((turn) => turn.reply)} />, optionsHost) : null}
    {hideChatList ? null : <div className="shared-assistant-toolbar">
      <strong title={activeConversation?.title ?? "New chat"}>{activeConversation?.title ?? "New chat"}</strong>
      {hideChatList ? null : <>
        <IconButton aria-expanded={showChats} label="Chat history" onClick={() => setShowChats((current) => !current)} size="sm"><History size={ICON_SIZE.md} /></IconButton>
        <IconButton disabled={isBusy} label="New chat" onClick={() => { startNewChat(); setShowChats(false); }} size="sm"><Plus size={ICON_SIZE.md} /></IconButton>
      </>}
    </div>}
    {showChats && !hideChatList ? <nav className="shared-assistant-chats" aria-label={`${ASSISTANT_NAME} chats`}>
      {conversations.length ? <ul className="rm-option-list">{conversations.map((conversation) => {
        const isActive = conversation.id === activeId;
        return <li className={isActive ? "rm-option-row active" : "rm-option-row"} key={conversation.id}>
          <button aria-current={isActive ? "page" : undefined} className="rm-option" disabled={isBusy} onClick={() => { setShowChats(false); void openChat(conversation.id); }} title={conversation.title} type="button">
            <Message size={ICON_SIZE.sm} />
            <span className="rm-option-text"><strong>{conversation.title}</strong><small>{formatRecentTime(conversation.updated_at)} · {conversation.turn_count} {conversation.turn_count === 1 ? "message" : "messages"}</small></span>
          </button>
          <ActionMenu items={[
            { label: "Rename", icon: <Pencil size={ICON_SIZE.sm} />, onSelect: () => { setRenameValue(conversation.title); setChatDialog({ kind: "rename", conversation }); } },
            { label: "Delete chat", icon: <Trash size={ICON_SIZE.sm} />, destructive: true, onSelect: () => setChatDialog({ kind: "delete", conversation }) },
          ]} label={`Actions for chat ${conversation.title}`} />
        </li>;
      })}</ul> : <p className="shared-assistant-chats-empty">Your chats appear here. Only you can see them.</p>}
    </nav> : null}
    <div className="shared-assistant-main">
      <div className="shared-assistant-thread" role="log" aria-live="polite" aria-busy={isBusy || isLoadingChat}>
        {isLoadingChat ? <LoadingState label="Loading chat" /> : turns.length ? turns.map((turn) => <article className="shared-assistant-turn" key={turn.key}>
          <div className="shared-assistant-request"><p>{turn.label}</p>{turn.request.capability ? <span>{capabilityLabel(capabilities, turn.request.capability)}</span> : null}</div>
          {turn.status === "pending" ? <LoadingState label="Working on it" /> : null}
          {turn.status === "failed" ? <div className="shared-assistant-failed"><span>No reply. The request did not complete and was not saved.</span><Button disabled={isBusy} onClick={() => void run(turn)} variant="secondary"><Refresh size={ICON_SIZE.sm} /> Retry</Button></div> : null}
          {turn.reply ? <Reply accessToken={accessToken} canSummarize={canSummarize} capabilities={capabilities} canManage={canUpload} deciding={deciding} isBusy={isBusy} onActOnSelection={(action) => actOnSelection(turn.key, action)} onSelection={(ids) => setSelection((current) => ({ ...current, [turn.key]: ids }))} selection={selection[turn.key] ?? []} onDecide={(proposal, action) => void decide(turn.key, proposal, action)} onOpenArtifact={onOpenArtifact} onSummarize={summarize} reply={turn.reply} turnId={turn.key} /> : null}
        </article>) : variant === "page"
          ? <div className="shared-assistant-welcome">
            <CairnIcon size={56} weight="fill" />
            <strong>How can I help?</strong>
          </div>
          : <div className="shared-assistant-intro">
          <CairnIcon size={22} />
          <strong>Hi, I&apos;m {ASSISTANT_NAME}. What do you need from this workspace?</strong>
          <span>Pick an action below or just type. Requests are matched to one of these actions; anything else gets a short reply for now. Chats are saved so you can come back to them.</span>
        </div>}
        <div ref={threadEnd} />
      </div>
      {variant === "page" ? <form className="shared-assistant-composer shared-assistant-composer-page" onSubmit={submit}>
        {composerHeader}
        {!draft.trim() && !turns.length && suggestedQuestions(template?.assistant).length ? <div aria-label="Suggested questions" className="shared-assistant-suggestions" role="group">
          {suggestedQuestions(template?.assistant).map((question) => <Button disabled={isBusy || isLoadingChat} key={question} onClick={() => { setDraft(question); input.current?.focus(); }} type="button" variant="secondary">{question}</Button>)}
        </div> : null}
        <input aria-label="Attach files" className="shared-upload-native-input" hidden multiple onChange={(event) => { const chosen = Array.from(event.target.files ?? []); event.target.value = ""; void uploadFiles(chosen); }} ref={fileInput} type="file" />
        <div className="shared-assistant-box">
          <MenuPrimitive.Root>
            <MenuPrimitive.Trigger asChild>
              <IconButton appearance="plain" className="shared-assistant-more" label="Actions" size="sm" type="button"><Plus size={ICON_SIZE.lg} /></IconButton>
            </MenuPrimitive.Trigger>
            <MenuPrimitive.Portal>
              <MenuPrimitive.Content align="start" aria-label={`${ASSISTANT_NAME} actions`} className="rm-action-menu" side="top" sideOffset={8}>
                {canUpload ? <MenuPrimitive.Item className="rm-action-item" disabled={isBusy || isLoadingChat} onSelect={() => fileInput.current?.click()} title="Upload files into this workspace. They are indexed, so I can search and answer from them.">
                  <Paperclip size={ICON_SIZE.md} /><span>Attach files</span>
                </MenuPrimitive.Item> : null}
                {orderCapabilities(capabilities?.capabilities ?? [], template?.assistant).map((capability) => {
                  const Icon = CAPABILITY_ICONS[capability.id];
                  return <MenuPrimitive.Item className="rm-action-item" disabled={!capability.available || isBusy || isLoadingChat} key={capability.id} onSelect={() => choose(capability.id)} title={capability.available ? capability.description : `${capability.description} Needs an AI provider enabled in Settings.`}>
                    <Icon size={ICON_SIZE.md} weight={selected === capability.id ? "fill" : "regular"} /><span>{capability.label}</span>
                  </MenuPrimitive.Item>;
                })}
              </MenuPrimitive.Content>
            </MenuPrimitive.Portal>
          </MenuPrimitive.Root>
          <div className="shared-assistant-box-field">
            {selectedInfo ? <span className="shared-assistant-chip">{selectedInfo.label}<button aria-label={`Stop using ${selectedInfo.label}`} onClick={() => setSelected(null)} type="button"><Clear size={ICON_SIZE.sm} /></button></span> : null}
            <Textarea aria-label={`Message ${ASSISTANT_NAME}`} onChange={(event) => setDraft(event.target.value)} onKeyDown={onComposerKey} placeholder={selectedInfo?.placeholder ?? "Ask, search, or name a file…"} ref={input} rows={1} value={draft} />
          </div>
          <IconButton appearance="plain" busy={isBusy} className="shared-assistant-send" disabled={isLoadingChat || !draft.trim()} label="Send" size="sm" type="submit"><ArrowElbowDownLeft size={ICON_SIZE.lg} /></IconButton>
        </div>
        <p className="shared-assistant-model">{capabilities ? (capabilities.provider_name ? `Model: ${capabilities.model ?? "provider default"} · ${capabilities.provider_name}` : "No AI provider: file lookup and search only") : ""}</p>
      </form> : <form className="shared-assistant-composer" onSubmit={submit}>
        <div className="shared-assistant-actions" role="group" aria-label={`${ASSISTANT_NAME} actions`}>
          {orderCapabilities(capabilities?.capabilities ?? [], template?.assistant).map((capability) => {
            const Icon = CAPABILITY_ICONS[capability.id];
            return <Button aria-pressed={selected === capability.id} className={selected === capability.id ? "active" : ""} disabled={!capability.available || isBusy || isLoadingChat} key={capability.id} onClick={() => choose(capability.id)} title={capability.available ? capability.description : `${capability.description} Needs an AI provider enabled in Settings.`} type="button" variant="secondary"><Icon size={ICON_SIZE.sm} />{capability.label}</Button>;
          })}
        </div>
        {!draft.trim() && !turns.length && suggestedQuestions(template?.assistant).length ? <div aria-label="Suggested questions" className="shared-assistant-suggestions" role="group">
          {suggestedQuestions(template?.assistant).map((question) => <Button disabled={isBusy || isLoadingChat} key={question} onClick={() => { setDraft(question); input.current?.focus(); }} type="button" variant="secondary">{question}</Button>)}
        </div> : null}
        <div className="shared-assistant-input">
          <Textarea aria-label={`Message ${ASSISTANT_NAME}`} onChange={(event) => setDraft(event.target.value)} onKeyDown={onComposerKey} placeholder={selectedInfo?.placeholder ?? "Ask, search, or name a file…"} ref={input} rows={2} value={draft} />
          <Button aria-label="Send" busy={isBusy} disabled={isLoadingChat || !draft.trim()} type="submit" variant="main"><Send size={ICON_SIZE.md} /></Button>
        </div>
        <div className="shared-assistant-footnote">
          <span>Enter or {submitShortcutLabel()} sends; Shift+Enter adds a line.</span>
          <span>{capabilities?.provider_name ? <><Brain size={ICON_SIZE.sm} /> AI actions use {capabilities.provider_name}. Search and file lookup stay on the server.</> : "No AI provider is enabled, so file lookup and search are available. An administrator can enable AI in Settings."}</span>
        </div>
      </form>}
    </div>
  </section>;
}

function capabilityLabel(capabilities: AgentCapabilities | null, id: AgentCapability) {
  return capabilities?.capabilities.find((capability) => capability.id === id)?.label ?? id.replace(/_/g, " ");
}

function routingNote(reply: AgentReply, capabilities: AgentCapabilities | null) {
  if (!reply.capability) return null;
  const label = capabilityLabel(capabilities, reply.capability);
  if (reply.routing === "rules") return `${label} · matched from your wording`;
  if (reply.routing === "model") return `${label} · interpreted by ${capabilities?.provider_name ?? "the AI provider"}`;
  return label;
}

const EVIDENCE_PREVIEW_LIMIT = 5;

function Evidence({ accessToken, citations, isQuestion, onOpenArtifact }: { accessToken: string; citations: AgentReply["citations"]; isQuestion: boolean; onOpenArtifact: (artifactId: string) => void }) {
  const [showAll, setShowAll] = useState(false);
  const hidden = citations.length - EVIDENCE_PREVIEW_LIMIT;
  const renderCitation = (citation: AgentReply["citations"][number], index: number, onOpen: (artifactId: string) => void) => <span className="shared-assistant-citation-row" key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}`}>
    <button className="shared-assistant-citation" onClick={() => onOpen(citation.artifact_id)} type="button">{/* Answers cite their passages as [1], [2]… in this order. */}{isQuestion ? `[${index + 1}] ` : ""}{citation.title} · {citation.path}{citation.start_line ? ` · line ${citation.start_line}` : ""}</button>
    {citationChange(citation) ? <small className="shared-citation-change">{citationChange(citation)}</small> : null}
    <ViewAtCommit accessToken={accessToken} citation={citation} />
  </span>;
  return <div className="shared-ai-citations">
    <strong>What {ASSISTANT_NAME} read</strong>
    {citations.slice(0, EVIDENCE_PREVIEW_LIMIT).map((citation, index) => renderCitation(citation, index, onOpenArtifact))}
    {hidden > 0 ? <button className="shared-assistant-citation-more" onClick={() => setShowAll(true)} type="button">Show all {citations.length} sources</button> : null}
    {showAll ? <Dialog size="lg" footer={<DialogCancel label="Close" onClick={() => setShowAll(false)} />} onClose={() => setShowAll(false)} open title={`What ${ASSISTANT_NAME} read (${citations.length})`}>
      <div className="shared-ai-citations shared-ai-citations-full">{citations.map((citation, index) => renderCitation(citation, index, (id) => { setShowAll(false); onOpenArtifact(id); }))}</div>
    </Dialog> : null}
  </div>;
}

/** What CAIRN proposes to do: shown before anything happens, with the person's *Confirm* and *Dismiss*, and afterwards with what was done. */
function Proposal({ deciding, onDecide, proposal, turnId }: { deciding: string | null; onDecide: (proposal: AgentProposal, action: "confirm" | "dismiss") => void; proposal: AgentProposal; turnId: string }) {
  const waiting = deciding === `${turnId}/${proposal.id}`;
  return <aside aria-label={proposal.title} className={`shared-assistant-proposal shared-assistant-proposal-${proposal.state}`}>
    <strong>{proposal.title}</strong>
    <div className="shared-markdown"><SafeMarkdown>{proposal.detail}</SafeMarkdown></div>
    {proposal.state === "pending" ? <div className="shared-assistant-proposal-actions">
      <Button busy={waiting} disabled={deciding !== null} onClick={() => onDecide(proposal, "confirm")} type="button" variant="main">Confirm</Button>
      <Button disabled={deciding !== null} onClick={() => onDecide(proposal, "dismiss")} type="button" variant="secondary">Dismiss</Button>
    </div> : <small>{proposal.state === "applied" ? proposal.outcome ?? "Done." : "Dismissed. Nothing was changed."}</small>}
  </aside>;
}

function Reply({ accessToken, canManage, canSummarize, capabilities, deciding, isBusy, onActOnSelection, onDecide, onOpenArtifact, onSelection, onSummarize, reply, selection, turnId }: { accessToken: string; canManage: boolean; canSummarize: boolean; capabilities: AgentCapabilities | null; deciding: string | null; isBusy: boolean; onActOnSelection: (action: "move" | "trash") => void; onSelection: (ids: string[]) => void; selection: string[]; onDecide: (proposal: AgentProposal, action: "confirm" | "dismiss") => void; onOpenArtifact: (artifactId: string) => void; onSummarize: (artifactId: string, title: string) => void; reply: AgentReply; turnId: string }) {
  const note = routingNote(reply, capabilities);
  const earlier = reply.context_turns ?? 0;
  // Found files, or the choices offered when a file name was ambiguous.
  const offerSummary = canSummarize && !(reply.capability === "summarize_file" && reply.generated);
  return <div className="shared-assistant-reply">
    <div className="shared-assistant-reply-meta">
      <span><CairnIcon size={ICON_SIZE.sm} /> {ASSISTANT_NAME}{note ? ` · ${note}` : ""}{earlier > 0 ? ` · read ${earlier} earlier ${earlier === 1 ? "message" : "messages"}` : ""}</span>
      {reply.generated ? <span className="shared-assistant-generated">AI-generated · verify against the cited sources</span> : null}
    </div>
    <div className="shared-markdown"><SafeMarkdown>{reply.reply_markdown}</SafeMarkdown></div>
    {reply.warnings.length ? <div className="shared-ai-overview-warning">{reply.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}
    {reply.file_set && reply.file_set.ids.length > 1 ? <div className="shared-assistant-selectbar">
      <span>{reply.file_set.description}: {reply.file_set.total}{reply.file_set.total > reply.files.length ? ` (showing ${reply.files.length})` : ""}</span>
      {canManage ? <>
        <Button disabled={isBusy} onClick={() => onSelection(selection.length === reply.file_set!.ids.length ? [] : reply.file_set!.ids)} type="button" variant="secondary">{selection.length === reply.file_set.ids.length ? "Clear selection" : `Select all ${reply.file_set.ids.length}`}</Button>
        {selection.length ? <>
          <span>{selection.length} selected</span>
          <Button disabled={isBusy} onClick={() => onActOnSelection("move")} type="button" variant="secondary">Move to…</Button>
          <Button disabled={isBusy} onClick={() => onActOnSelection("trash")} type="button" variant="secondary">Move to trash</Button>
        </> : null}
      </> : null}
    </div> : null}
    {reply.files.length ? <ul className="shared-assistant-files">{reply.files.map((file) => <li key={file.id}>
      {reply.file_set && reply.file_set.ids.length > 1 && canManage ? <input aria-label={`Select ${file.title}`} checked={selection.includes(file.id)} className="shared-assistant-select" disabled={isBusy} onChange={(event) => onSelection(event.target.checked ? [...selection, file.id] : selection.filter((id) => id !== file.id))} type="checkbox" /> : null}
      <Button className="shared-assistant-file" onClick={() => onOpenArtifact(file.id)} type="button" variant="secondary"><FileText size={ICON_SIZE.md} /><span><strong>{file.title}</strong><small>{file.path}</small></span></Button>
      {offerSummary ? <Button disabled={isBusy || !file.indexed_at} onClick={() => onSummarize(file.id, file.title)} title={file.indexed_at ? `Summarize ${file.title}` : "This file is not indexed yet"} type="button" variant="secondary">Summarize</Button> : null}
    </li>)}</ul> : null}
    {reply.matches.length ? <div className="shared-search-results">{reply.matches.map((match) => <article key={match.chunk_id}><Button className="shared-search-result" onClick={() => onOpenArtifact(match.artifact_id)} type="button" variant="secondary"><span className="shared-search-result-heading"><strong>{match.title}</strong><span>{match.source_name}</span></span><p>{match.snippet}</p><span className="shared-search-result-path">{match.path}{match.start_line ? ` · line ${match.start_line}` : ""}</span></Button></article>)}</div> : null}
    {(reply.proposals ?? []).map((proposal) => <Proposal deciding={deciding} key={proposal.id} onDecide={onDecide} proposal={proposal} turnId={turnId} />)}
    {reply.citations.length ? <Evidence accessToken={accessToken} isQuestion={reply.capability === "ask_question"} citations={reply.citations} onOpenArtifact={onOpenArtifact} /> : null}
  </div>;
}
