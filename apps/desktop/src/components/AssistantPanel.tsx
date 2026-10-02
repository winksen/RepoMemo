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
  IconMessageQuestion as Question,
  IconRefresh as Refresh,
  IconSearch as Search,
  IconSend as Send,
  IconSparkles as Sparkles,
} from "@tabler/icons-react";
import { getSharedAgentCapabilities, sendSharedAgentMessage } from "../lib/sharedApi";
import type { AgentCapabilities, AgentCapability, AgentMessage, AgentReply } from "../types";
import { Button } from "./ui/button";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";

type Turn = {
  id: number;
  request: AgentMessage;
  /** What the user sees as their message; picked actions have no typed text. */
  label: string;
  status: "pending" | "done" | "failed";
  reply?: AgentReply;
};

const CAPABILITY_ICONS: Record<AgentCapability, typeof Search> = {
  find_files: FileSearch,
  search_content: Search,
  summarize_file: FileText,
  ask_question: Question,
  workspace_overview: Dashboard,
};

const STORAGE_PREFIX = "repomemo.assistant.";

function loadTurns(workspaceId: string): Turn[] {
  try {
    const stored = window.sessionStorage.getItem(STORAGE_PREFIX + workspaceId);
    const turns = stored ? (JSON.parse(stored) as Turn[]) : [];
    // A reply still pending when the page was left will never arrive.
    return turns.filter((turn) => turn.status === "done");
  } catch {
    return [];
  }
}

function saveTurns(workspaceId: string, turns: Turn[]) {
  try {
    window.sessionStorage.setItem(STORAGE_PREFIX + workspaceId, JSON.stringify(turns.filter((turn) => turn.status === "done").slice(-30)));
  } catch { /* the conversation still works for this visit */ }
}

/** Workspace assistant: routes each message to one capability on the server. */
export function AssistantPanel({ accessToken, onOpenArtifact, workspaceId }: { accessToken: string; onOpenArtifact: (artifactId: string) => void; workspaceId: string }) {
  const [capabilities, setCapabilities] = useState<AgentCapabilities | null>(null);
  const [selected, setSelected] = useState<AgentCapability | null>(null);
  const [draft, setDraft] = useState("");
  const [turns, setTurns] = useState<Turn[]>(() => loadTurns(workspaceId));
  const nextId = useRef(Date.now());
  const threadEnd = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const isBusy = turns.some((turn) => turn.status === "pending");
  const selectedInfo = capabilities?.capabilities.find((capability) => capability.id === selected) ?? null;
  const canSummarize = capabilities?.capabilities.some((capability) => capability.id === "summarize_file" && capability.available) ?? false;

  useEffect(() => {
    let cancelled = false;
    getSharedAgentCapabilities(accessToken, workspaceId)
      .then((next) => { if (!cancelled) setCapabilities(next); })
      .catch((error: unknown) => showToast("error", error instanceof Error ? error.message : "The assistant is unavailable."));
    return () => { cancelled = true; };
  }, [accessToken, workspaceId]);
  useEffect(() => { saveTurns(workspaceId, turns); }, [turns, workspaceId]);
  useEffect(() => { threadEnd.current?.scrollIntoView({ block: "end", behavior: "smooth" }); }, [turns.length, isBusy]);

  async function run(turn: Turn) {
    setTurns((current) => [...current.filter((entry) => entry.id !== turn.id), { ...turn, status: "pending", reply: undefined }]);
    try {
      const reply = await sendSharedAgentMessage(accessToken, workspaceId, turn.request);
      setTurns((current) => current.map((entry) => entry.id === turn.id ? { ...entry, status: "done", reply } : entry));
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "The assistant could not reply.");
      setTurns((current) => current.map((entry) => entry.id === turn.id ? { ...entry, status: "failed" } : entry));
    }
  }

  function send(request: AgentMessage, label: string) {
    if (isBusy) return;
    void run({ id: nextId.current++, request, label, status: "pending" });
  }

  function submit(event?: FormEvent<HTMLFormElement>) {
    event?.preventDefault();
    const message = draft.trim();
    if (!message) return;
    send({ message, capability: selected ?? undefined }, message);
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
      send({ message: "", capability }, "Give me a workspace overview");
      setSelected(null);
      return;
    }
    setSelected((current) => current === capability ? null : capability);
    input.current?.focus();
  }

  function summarize(artifactId: string, title: string) {
    send({ message: title, capability: "summarize_file", artifact_id: artifactId }, `Summarize ${title}`);
  }

  function clearConversation() {
    setTurns([]);
  }

  return <section className="shared-assistant" aria-label="Workspace assistant">
    <div className="shared-assistant-thread" role="log" aria-live="polite" aria-busy={isBusy}>
      {turns.length ? turns.map((turn) => <article className="shared-assistant-turn" key={turn.id}>
        <div className="shared-assistant-request"><p>{turn.label}</p>{turn.request.capability ? <span>{capabilityLabel(capabilities, turn.request.capability)}</span> : null}</div>
        {turn.status === "pending" ? <p className="shared-assistant-pending"><Loader className="spin" size={16} /> Working on it…</p> : null}
        {turn.status === "failed" ? <div className="shared-assistant-failed"><span>No reply. The request did not complete.</span><Button disabled={isBusy} onClick={() => void run(turn)} type="button" variant="secondary"><Refresh size={15} /> Retry</Button></div> : null}
        {turn.reply ? <Reply canSummarize={canSummarize} capabilities={capabilities} isBusy={isBusy} onOpenArtifact={onOpenArtifact} onSummarize={summarize} reply={turn.reply} /> : null}
      </article>) : <div className="shared-assistant-intro">
        <Sparkles size={22} />
        <strong>What do you need from this workspace?</strong>
        <span>Pick an action below or just type. Requests are matched to one of these actions; anything else gets a short reply for now.</span>
      </div>}
      <div ref={threadEnd} />
    </div>
    <form className="shared-assistant-composer" onSubmit={submit}>
      <div className="shared-assistant-actions" role="group" aria-label="Assistant actions">
        {(capabilities?.capabilities ?? []).map((capability) => {
          const Icon = CAPABILITY_ICONS[capability.id];
          return <Button aria-pressed={selected === capability.id} className={selected === capability.id ? "active" : ""} disabled={!capability.available || isBusy} key={capability.id} onClick={() => choose(capability.id)} title={capability.available ? capability.description : `${capability.description} Needs an AI provider enabled in Settings.`} type="button" variant="secondary"><Icon size={15} />{capability.label}</Button>;
        })}
      </div>
      <div className="shared-assistant-input">
        <Textarea aria-label="Message the assistant" onChange={(event) => setDraft(event.target.value)} onKeyDown={onComposerKey} placeholder={selectedInfo?.placeholder ?? "Ask, search, or name a file…"} ref={input} rows={2} value={draft} />
        <Button aria-label="Send" disabled={isBusy || !draft.trim()} type="submit" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <Send size={16} />}</Button>
      </div>
      <div className="shared-assistant-footnote">
        <span>{capabilities?.provider_name ? <><Brain size={13} /> AI actions use {capabilities.provider_name}. Search and file lookup stay on the server.</> : "No AI provider is enabled, so file lookup and search are available. An administrator can enable AI in Settings."}</span>
        {turns.length ? <button disabled={isBusy} onClick={clearConversation} type="button">Clear conversation</button> : null}
      </div>
    </form>
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
    {reply.citations.length ? <div className="shared-ai-citations"><strong>Evidence used</strong>{reply.citations.map((citation) => <button className="shared-assistant-citation" key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}`} onClick={() => onOpenArtifact(citation.artifact_id)} type="button">{citation.title} · {citation.path}{citation.start_line ? ` · line ${citation.start_line}` : ""}</button>)}</div> : null}
  </div>;
}
