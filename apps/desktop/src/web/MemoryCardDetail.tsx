import { type FormEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { SafeMarkdown } from "../components/SafeMarkdown";
import { IconArrowLeft as ArrowLeft, IconFileText as FileText, IconLoader2 as Loader, IconMarkdown as Markdown, IconPencil as Pencil, IconTrash as Trash } from "@tabler/icons-react";
import { deleteSharedMemoryCard, exportSharedMemoryCard, getSharedMemoryCard, listSharedArtifacts, updateSharedMemoryCard } from "../lib/sharedApi";
import { type ArtifactSummary, type MemoryCardDetail, type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Textarea } from "../components/ui/textarea";
import { Toast } from "../components/ui/toast";
import { SharedLayout } from "../components/SharedLayout";
import { invalidateKeys, queryClient, workspaceKeys } from "../lib/queryClient";
import { useLiveEventHub, useLiveEvents } from "../lib/liveEvents";
import { useServerState } from "../lib/useServerState";
import { LEDGER_ACTIONS, LIVE_REFRESH_DELAY_MS, type WorkspaceSection } from "./constants";
import { apiMessage, citationLines } from "./format";
import { navigate } from "./router";
import { OrganizationRail, WorkspaceTopbar } from "./Organization";

type EditableCitation = {
  artifact_id: string;
  chunk_id: string | null;
  title: string;
  path: string;
  start_line: number | null;
  end_line: number | null;
  confidence: number | null;
};

export function SharedMemoryCardDetail({
  accessToken,
  apiAvailable,
  cardId,
  onBack,
  onOpenArtifact,
  organization,
  organizations,
  session,
  signOut,
  workspace,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  cardId: string;
  onBack: () => void;
  onOpenArtifact: (artifactId: string) => void;
  organization?: Organization;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspace: SharedWorkspace;
  workspaces: SharedWorkspace[];
}) {
  // The card is in the query cache under its workspace: shown at once on a
  // later visit, refetched when the page opens, and marked stale by live events.
  const workspaceId = workspace.workspace.id;
  const cardKey = workspaceKeys.memoryCard(workspaceId, cardId);
  const [card, , cardQuery] = useServerState<MemoryCardDetail | null>({
    key: cardKey, fetch: () => getSharedMemoryCard(accessToken, cardId), initial: null, staleTime: 0,
  });
  const [error, setError] = useState<string | null>(null);
  const [isReloading, setIsReloading] = useState(false);
  const isLoading = isReloading || cardQuery.isLoading;
  const [isExporting, setIsExporting] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isMutating, setIsMutating] = useState(false);
  const [title, setTitle] = useState("");
  const [bodyMarkdown, setBodyMarkdown] = useState("");
  const [source, setSource] = useState("");
  // Evidence being edited: what the card will cite once saved.
  const [citations, setCitations] = useState<EditableCitation[]>([]);
  const [addArtifactId, setAddArtifactId] = useState("");
  const canWrite = workspace.role !== "viewer";
  // The evidence a card can cite is the workspace's file list, read when editing starts.
  const [artifacts] = useServerState<ArtifactSummary[]>({
    key: workspaceKeys.artifacts(workspaceId), fetch: () => listSharedArtifacts(accessToken, workspaceId), initial: [], enabled: isEditing,
  });

  async function load() {
    setIsReloading(true); setError(null);
    try { await invalidateKeys([cardKey]); } finally { setIsReloading(false); }
  }
  useEffect(() => { if (cardQuery.error) setError(apiMessage(cardQuery.error)); }, [cardQuery.error]);

  // Live updates: another person editing this card, or evidence it cites
  // changing (which can make it need review), refreshes it once per burst.
  const liveHub = useLiveEventHub(`/v1/workspaces/${encodeURIComponent(workspaceId)}/events`, accessToken);
  const refreshTimer = useRef<number | null>(null);
  useEffect(() => () => { if (refreshTimer.current !== null) window.clearTimeout(refreshTimer.current); }, []);
  useLiveEvents((event) => {
    const affectsCard = event.type === "resync"
      || (event.type === "activity" && (
        (event.event.action === "memory_updated" && event.event.subject_id === cardId && event.event.actor?.id !== session.user.id)
        || LEDGER_ACTIONS.has(event.event.action)));
    if (!affectsCard || refreshTimer.current !== null) return;
    refreshTimer.current = window.setTimeout(() => {
      refreshTimer.current = null;
      void queryClient.invalidateQueries({ queryKey: cardKey });
    }, LIVE_REFRESH_DELAY_MS);
  }, liveHub);
  useEffect(() => {
    setTitle(card?.card.title ?? "");
    setBodyMarkdown(card?.card.body_markdown ?? "");
    setSource(card?.card.source ?? "");
  }, [card?.card.body_markdown, card?.card.source, card?.card.title]);

  function startEditing() {
    setCitations((card?.evidence ?? []).filter((entry) => entry.artifact_id && entry.exists).map((entry) => ({
      artifact_id: entry.artifact_id!,
      chunk_id: entry.chunk_id,
      title: entry.title ?? "Untitled evidence",
      path: entry.path ?? "",
      start_line: entry.start_line,
      end_line: entry.end_line,
      confidence: null,
    })));
    setIsEditing(true);
  }

  function addCitation() {
    const artifact = artifacts.find((entry) => entry.id === addArtifactId);
    if (!artifact || citations.some((entry) => entry.artifact_id === artifact.id && entry.chunk_id === null)) return;
    setCitations((current) => [...current, { artifact_id: artifact.id, chunk_id: null, title: artifact.title, path: artifact.path, start_line: null, end_line: null, confidence: null }]);
    setAddArtifactId("");
  }

  /** Saves the card as it is: its evidence is still what it says, so it stops asking for review. */
  async function confirmCard() {
    if (!card) return;
    setIsMutating(true); setError(null);
    try {
      await updateSharedMemoryCard(accessToken, cardId, { title: card.card.title, bodyMarkdown: card.card.body_markdown, source: card.card.source });
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function exportCard() {
    setIsExporting(true); setError(null);
    try {
      const markdown = await exportSharedMemoryCard(accessToken, cardId);
      const file = new Blob([markdown], { type: "text/markdown;charset=utf-8" });
      const url = URL.createObjectURL(file);
      const link = document.createElement("a");
      link.href = url;
      link.download = `${(card?.card.title ?? "repomemo-memory").replace(/[^a-z0-9-_]+/gi, "-").toLowerCase()}.md`;
      link.click();
      URL.revokeObjectURL(url);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsExporting(false); }
  }

  async function saveCard(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsMutating(true); setError(null);
    try {
      await updateSharedMemoryCard(accessToken, cardId, { title, bodyMarkdown, source, citations });
      setIsEditing(false);
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function removeCard() {
    if (!window.confirm(`Delete ${card?.card.title ?? "this memory card"}? This cannot be undone.`)) return;
    setIsMutating(true); setError(null);
    try { await deleteSharedMemoryCard(accessToken, cardId); onBack(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  return (
    <SharedRecordLayout activeSection="memory" apiAvailable={apiAvailable} organization={organization} organizations={organizations} session={session} signOut={signOut} title={card?.card.title ?? "Memory card"} subtitle={card ? `Source: ${card.card.source}` : "Loading durable team memory…"} workspace={workspace} workspaces={workspaces}>
      <div className="shared-detail-actions">{canWrite ? <><Button disabled={isLoading || isMutating} onClick={() => (isEditing ? setIsEditing(false) : startEditing())} type="button" variant="secondary"><Pencil size={16} /> {isEditing ? "Cancel edit" : "Edit"}</Button><Button className="shared-danger-action" disabled={isLoading || isMutating} onClick={() => void removeCard()} type="button" variant="secondary"><Trash size={16} /> Delete</Button></> : null}<Button disabled={isLoading || isExporting || isMutating} onClick={() => void exportCard()} type="button" variant="main">{isExporting ? <Loader className="spin" size={16} /> : <FileText size={16} />} Export Markdown</Button></div>
      <Toast kind="error" message={error} />
      {card?.review.length ? <section className="shared-record-panel shared-review-panel"><h2>Needs review</h2><p className="shared-muted-copy">Evidence this card cites has changed since it was written. Check what it says, edit it or its evidence, or confirm it as it is.</p><ul>{card.review.map((reason, index) => <li key={`${reason.kind}-${index}`}>{reason.detail}{reason.artifact_id ? <> <button className="shared-assistant-citation" onClick={() => onOpenArtifact(reason.artifact_id!)} type="button">Open</button></> : null}</li>)}</ul>{canWrite ? <Button disabled={isMutating} onClick={() => void confirmCard()} type="button" variant="secondary">Confirm as it is</Button> : null}</section> : null}
      <section className="shared-record-panel"><h2>Durable statement</h2>{isEditing ? <form className="shared-record-edit-form" onSubmit={saveCard}><label>Title<Input onChange={(event) => setTitle(event.target.value)} required value={title} /></label><label>Source<Input onChange={(event) => setSource(event.target.value)} required value={source} /></label><label>Statement<Textarea onChange={(event) => setBodyMarkdown(event.target.value)} required value={bodyMarkdown} /></label><fieldset className="shared-memory-citations"><legend>Evidence this card cites</legend>{citations.length ? <ul>{citations.map((citation, index) => <li key={`${citation.artifact_id}-${citation.chunk_id ?? "file"}-${index}`}><span>{citation.title}{citation.chunk_id ? citationLines(citation) : " (whole file)"}</span><Button onClick={() => setCitations((current) => current.filter((_, position) => position !== index))} type="button" variant="secondary">Remove</Button></li>)}</ul> : <p className="shared-muted-copy">No evidence is cited.</p>}<div className="shared-memory-citation-add"><select aria-label="Evidence to add" onChange={(event) => setAddArtifactId(event.target.value)} value={addArtifactId}><option value="">Add a file…</option>{artifacts.filter((artifact) => !artifact.repository_id).map((artifact) => <option key={artifact.id} value={artifact.id}>{artifact.title}</option>)}</select><Button disabled={!addArtifactId} onClick={addCitation} type="button" variant="secondary">Add</Button></div></fieldset><Button disabled={isMutating} type="submit" variant="main">{isMutating ? <Loader className="spin" size={16} /> : <Pencil size={16} />} Save memory</Button></form> : <div className="shared-markdown shared-memory-body"><SafeMarkdown>{card?.card.body_markdown ?? ""}</SafeMarkdown></div>}</section>
      <section className="shared-record-panel"><h2>Linked evidence</h2>{card?.evidence.length ? <div className="shared-evidence-links">{card.evidence.map((evidence) => <article key={evidence.link_id}>{evidence.artifact_id && evidence.exists ? <button className="shared-assistant-citation shared-evidence-link-title" onClick={() => onOpenArtifact(evidence.artifact_id!)} type="button"><strong>{evidence.title ?? "Untitled evidence"}</strong></button> : <strong>{evidence.title ?? "Untitled evidence"}{evidence.exists ? "" : " (missing)"}</strong>}<span>{evidence.path ?? evidence.target_id}{citationLines(evidence)}</span></article>)}</div> : <p className="shared-muted-copy">This memory card currently has no linked evidence.</p>}</section>
    </SharedRecordLayout>
  );
}

export function SharedRecordLayout({
  activeSection,
  apiAvailable,
  children,
  organization,
  organizations,
  session,
  signOut,
  subtitle,
  title,
  workspace,
  workspaces,
}: {
  activeSection: WorkspaceSection;
  apiAvailable: boolean | null;
  children: ReactNode;
  organization?: Organization;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  subtitle: string;
  title: string;
  workspace: SharedWorkspace;
  workspaces: SharedWorkspace[];
}) {
  return (
    <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizationId={organization?.id} organizations={organizations} workspaceId={workspace.workspace.id} workspaces={workspaces} />} workspaceNavigation={<WorkspaceTopbar activeSection={activeSection} onNavigate={(section) => navigate(`/workspaces/${encodeURIComponent(workspace.workspace.id)}/${section}`)} workspace={workspace} />}>
      <section className="shared-detail-shell">
        <div className="shared-detail-heading"><div><h1>{title}</h1><p className="shared-record-subtitle">{subtitle}</p></div></div>
        <div className="shared-record-content">{children}</div>
      </section>
    </SharedLayout>
  );
}

export function SharedRouteNotFound({
  apiAvailable,
  onBack,
  organization,
  organizations,
  session,
  signOut,
  workspace,
  workspaces,
}: {
  apiAvailable: boolean | null;
  onBack: () => void;
  organization?: Organization;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspace?: SharedWorkspace;
  workspaces?: SharedWorkspace[];
}) {
  return (
    <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizationId={organization?.id} organizations={organizations} workspaceId={workspace?.workspace.id} workspaces={workspaces} />} workspaceNavigation={workspace ? <WorkspaceTopbar workspace={workspace} /> : undefined}>
      <section className="shared-page-content shared-route-page" aria-label="Unknown shared route">
        <p className="shared-eyebrow">Shared workspace</p>
        <h1>This address has no shared view.</h1>
        <p className="shared-intro-copy">The requested route is not available, or the workspace is not in your current signed-in membership.</p>
        <Button className="shared-route-action" onClick={onBack} type="button" variant="main"><ArrowLeft size={17} /> Go to workspaces</Button>
      </section>
    </SharedLayout>
  );
}
