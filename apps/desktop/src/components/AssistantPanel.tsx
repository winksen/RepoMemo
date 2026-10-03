import type { FormEvent, KeyboardEvent } from "react";
import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  IconBrain as Brain,
  IconFileSearch as FileSearch,
  IconFileText as FileText,
  IconLayoutDashboard as Dashboard,
  IconLoader2 as Loader,
  IconMessage as Message,
  IconMessageQuestion as Question,
  IconPencil as Pencil,
  IconPlus as Plus,
  IconRefresh as Refresh,
  IconSearch as Search,
  IconSend as Send,
  IconSparkles as Sparkles,
  IconTrash as Trash,
} from "@tabler/icons-react";
import {
  deleteSharedAgentConversation,
  getSharedAgentCapabilities,
  getSharedAgentConversation,
  listSharedAgentConversations,
  renameSharedAgentConversation,
  sendSharedAgentMessage,
} from "../lib/sharedApi";
import type { AgentCapabilities, AgentCapability, AgentConversation, AgentMessage, AgentReply, AgentTurn } from "../types";
import { ActionMenu } from "./ui/action-menu";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";

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
  search_content: Search,
  summarize_file: FileText,
  ask_question: Question,
  workspace_overview: Dashboard,
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

function errorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

/** Workspace assistant: routes each message to one capability on the server and keeps the user's chats there. */
export function AssistantPanel({ accessToken, onOpenArtifact, workspaceId }: { accessToken: string; onOpenArtifact: (artifactId: string) => void; workspaceId: string }) {
  const [capabilities, setCapabilities] = useState<AgentCapabilities | null>(null);
  const [conversations, setConversations] = useState<AgentConversation[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [turns, setTurns] = useState<ThreadTurn[]>([]);
  const [isLoadingChat, setIsLoadingChat] = useState(false);
  const [selected, setSelected] = useState<AgentCapability | null>(null);
  const [draft, setDraft] = useState("");
  const [chatDialog, setChatDialog] = useState<ChatDialog | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [isSavingChat, setIsSavingChat] = useState(false);
  // Bumped whenever the visible chat changes, so a reply that arrives after
  // the user moved on is not written into the wrong thread.
  const view = useRef(0);
  const pendingCounter = useRef(0);
  const threadEnd = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const isBusy = turns.some((turn) => turn.status === "pending");
  const selectedInfo = capabilities?.capabilities.find((capability) => capability.id === selected) ?? null;
  const canSummarize = capabilities?.capabilities.some((capability) => capability.id === "summarize_file" && capability.available) ?? false;
  const activeConversation = conversations.find((conversation) => conversation.id === activeId) ?? null;

  useEffect(() => {
    let cancelled = false;
    Promise.all([getSharedAgentCapabilities(accessToken, workspaceId), listSharedAgentConversations(accessToken, workspaceId)])
      .then(([nextCapabilities, nextConversations]) => {
        if (cancelled) return;
        setCapabilities(nextCapabilities);
        setConversations(nextConversations);
        const last = rememberedChat(workspaceId);
        if (last && nextConversations.some((conversation) => conversation.id === last)) void openChat(last);
      })
      .catch((error: unknown) => showToast("error", errorMessage(error, "The assistant is unavailable.")));
    return () => { cancelled = true; };
  }, [accessToken, workspaceId]);
  useEffect(() => { threadEnd.current?.scrollIntoView({ block: "end", behavior: "smooth" }); }, [turns.length, isBusy]);

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
      showToast("error", errorMessage(error, "This chat could not be loaded."));
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
      }
      setTurns((current) => current.map((entry) => entry.key === turn.key ? storedTurn(saved) : entry));
    } catch (error) {
      showToast("error", errorMessage(error, "The assistant could not reply."));
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
    if (capability === "workspace_overview") {
      send({ message: "", capability }, "Workspace overview");
      setSelected(null);
      return;
    }
    setSelected((current) => current === capability ? null : capability);
    input.current?.focus();
  }

  function summarize(artifactId: string, title: string) {
    send({ message: title, capability: "summarize_file", artifact_id: artifactId }, `Summarize ${title}`);
  }

  async function renameChat(conversation: AgentConversation) {
    setIsSavingChat(true);
    try {
      upsertConversation(await renameSharedAgentConversation(accessToken, conversation.id, renameValue.trim()));
      setChatDialog(null);
    } catch (error) {
      showToast("error", errorMessage(error, "The chat could not be renamed."));
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
      showToast("error", errorMessage(error, "The chat could not be deleted."));
    } finally {
      setIsSavingChat(false);
    }
  }

  const dialog = chatDialog?.kind === "rename"
    ? <Dialog footer={<><DialogCancel onClick={() => setChatDialog(null)} /><Button disabled={isSavingChat || !renameValue.trim()} onClick={() => void renameChat(chatDialog.conversation)} type="button" variant="main">Rename</Button></>} onClose={() => setChatDialog(null)} open title="Rename chat">
      <Input aria-label="Chat name" autoFocus maxLength={120} onChange={(event) => setRenameValue(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && renameValue.trim()) void renameChat(chatDialog.conversation); }} value={renameValue} />
    </Dialog>
    : chatDialog?.kind === "delete"
      ? <Dialog description={<>Delete <strong>{chatDialog.conversation.title}</strong> and its {chatDialog.conversation.turn_count} {chatDialog.conversation.turn_count === 1 ? "message" : "messages"}? This cannot be undone.</>} footer={<><DialogCancel onClick={() => setChatDialog(null)} /><Button className="rm-button-danger" disabled={isSavingChat} onClick={() => void deleteChat(chatDialog.conversation)} type="button" variant="secondary">Delete chat</Button></>} onClose={() => setChatDialog(null)} open title="Delete chat" />
      : null;

  return <section className="shared-assistant" aria-label="Workspace assistant">
    {dialog}
    <nav className="shared-assistant-chats" aria-label="Assistant chats">
      <Button className="shared-assistant-new-chat" disabled={isBusy} onClick={startNewChat} type="button" variant="secondary"><Plus size={15} /> New chat</Button>
      {conversations.length ? <ul>{conversations.map((conversation) => {
        const isActive = conversation.id === activeId;
        return <li className={isActive ? "active" : ""} key={conversation.id}>
          <button aria-current={isActive ? "page" : undefined} disabled={isBusy} onClick={() => void openChat(conversation.id)} title={conversation.title} type="button">
            <Message size={15} />
            <span><strong>{conversation.title}</strong><small>{formatChatTime(conversation.updated_at)} · {conversation.turn_count} {conversation.turn_count === 1 ? "message" : "messages"}</small></span>
          </button>
          <ActionMenu items={[
            { label: "Rename", icon: <Pencil size={15} />, onSelect: () => { setRenameValue(conversation.title); setChatDialog({ kind: "rename", conversation }); } },
            { label: "Delete chat", icon: <Trash size={15} />, destructive: true, onSelect: () => setChatDialog({ kind: "delete", conversation }) },
          ]} label={`Actions for chat ${conversation.title}`} />
        </li>;
      })}</ul> : <p className="shared-assistant-chats-empty">Your chats appear here. Only you can see them.</p>}
    </nav>
    <div className="shared-assistant-main">
      <div className="shared-assistant-thread-title"><strong>{activeConversation?.title ?? "New chat"}</strong></div>
      <div className="shared-assistant-thread" role="log" aria-live="polite" aria-busy={isBusy || isLoadingChat}>
        {isLoadingChat ? <p className="shared-assistant-pending"><Loader className="spin" size={16} /> Loading chat…</p> : turns.length ? turns.map((turn) => <article className="shared-assistant-turn" key={turn.key}>
          <div className="shared-assistant-request"><p>{turn.label}</p>{turn.request.capability ? <span>{capabilityLabel(capabilities, turn.request.capability)}</span> : null}</div>
          {turn.status === "pending" ? <p className="shared-assistant-pending"><Loader className="spin" size={16} /> Working on it…</p> : null}
          {turn.status === "failed" ? <div className="shared-assistant-failed"><span>No reply. The request did not complete and was not saved.</span><Button disabled={isBusy} onClick={() => void run(turn)} type="button" variant="secondary"><Refresh size={15} /> Retry</Button></div> : null}
          {turn.reply ? <Reply canSummarize={canSummarize} capabilities={capabilities} isBusy={isBusy} onOpenArtifact={onOpenArtifact} onSummarize={summarize} reply={turn.reply} /> : null}
        </article>) : <div className="shared-assistant-intro">
          <Sparkles size={22} />
          <strong>What do you need from this workspace?</strong>
          <span>Pick an action below or just type. Requests are matched to one of these actions; anything else gets a short reply for now. Chats are saved so you can come back to them.</span>
        </div>}
        <div ref={threadEnd} />
      </div>
      <form className="shared-assistant-composer" onSubmit={submit}>
        <div className="shared-assistant-actions" role="group" aria-label="Assistant actions">
          {(capabilities?.capabilities ?? []).map((capability) => {
            const Icon = CAPABILITY_ICONS[capability.id];
            return <Button aria-pressed={selected === capability.id} className={selected === capability.id ? "active" : ""} disabled={!capability.available || isBusy || isLoadingChat} key={capability.id} onClick={() => choose(capability.id)} title={capability.available ? capability.description : `${capability.description} Needs an AI provider enabled in Settings.`} type="button" variant="secondary"><Icon size={15} />{capability.label}</Button>;
          })}
        </div>
        <div className="shared-assistant-input">
          <Textarea aria-label="Message the assistant" onChange={(event) => setDraft(event.target.value)} onKeyDown={onComposerKey} placeholder={selectedInfo?.placeholder ?? "Ask, search, or name a file…"} ref={input} rows={2} value={draft} />
          <Button aria-label="Send" disabled={isBusy || isLoadingChat || !draft.trim()} type="submit" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <Send size={16} />}</Button>
        </div>
        <div className="shared-assistant-footnote">
          <span>{capabilities?.provider_name ? <><Brain size={13} /> AI actions use {capabilities.provider_name}. Search and file lookup stay on the server.</> : "No AI provider is enabled, so file lookup and search are available. An administrator can enable AI in Settings."}</span>
        </div>
      </form>
    </div>
  </section>;
}

function formatChatTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const sameDay = date.toDateString() === new Date().toDateString();
  return sameDay
    ? date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })
    : date.toLocaleDateString(undefined, { day: "numeric", month: "short" });
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

function Reply({ canSummarize, capabilities, isBusy, onOpenArtifact, onSummarize, reply }: { canSummarize: boolean; capabilities: AgentCapabilities | null; isBusy: boolean; onOpenArtifact: (artifactId: string) => void; onSummarize: (artifactId: string, title: string) => void; reply: AgentReply }) {
  const note = routingNote(reply, capabilities);
  // Found files, or the choices offered when a file name was ambiguous.
  const offerSummary = canSummarize && !(reply.capability === "summarize_file" && reply.generated);
  return <div className="shared-assistant-reply">
    <div className="shared-assistant-reply-meta">
      <span><Sparkles size={14} /> Assistant{note ? ` · ${note}` : ""}</span>
      {reply.generated ? <span className="shared-assistant-generated">AI-generated · verify against the cited sources</span> : null}
    </div>
    <div className="shared-markdown"><ReactMarkdown remarkPlugins={[remarkGfm]}>{reply.reply_markdown}</ReactMarkdown></div>
    {reply.warnings.length ? <div className="shared-ai-overview-warning">{reply.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}
    {reply.files.length ? <ul className="shared-assistant-files">{reply.files.map((file) => <li key={file.id}>
      <Button className="shared-assistant-file" onClick={() => onOpenArtifact(file.id)} type="button" variant="secondary"><FileText size={16} /><span><strong>{file.title}</strong><small>{file.path}</small></span></Button>
      {offerSummary ? <Button disabled={isBusy || !file.indexed_at} onClick={() => onSummarize(file.id, file.title)} title={file.indexed_at ? `Summarize ${file.title}` : "This file is not indexed yet"} type="button" variant="secondary">Summarize</Button> : null}
    </li>)}</ul> : null}
    {reply.matches.length ? <div className="shared-search-results">{reply.matches.map((match) => <article key={match.chunk_id}><Button className="shared-search-result" onClick={() => onOpenArtifact(match.artifact_id)} type="button" variant="secondary"><span className="shared-search-result-heading"><strong>{match.title}</strong><span>{match.source_name}</span></span><p>{match.snippet}</p><span className="shared-search-result-path">{match.path}{match.start_line ? ` · line ${match.start_line}` : ""}</span></Button></article>)}</div> : null}
    {reply.citations.length ? <div className="shared-ai-citations"><strong>Evidence used</strong>{reply.citations.map((citation, index) => <button className="shared-assistant-citation" key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}`} onClick={() => onOpenArtifact(citation.artifact_id)} type="button">{/* Answers cite their passages as [1], [2]… in this order. */}{reply.capability === "ask_question" ? `[${index + 1}] ` : ""}{citation.title} · {citation.path}{citation.start_line ? ` · line ${citation.start_line}` : ""}</button>)}</div> : null}
  </div>;
}
