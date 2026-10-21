import { type FormEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { SafeMarkdown } from "../components/SafeMarkdown";
import { IconArrowLeft as ArrowLeft, IconFileText as FileText, IconMarkdown as Markdown, IconPencil as Pencil, IconTrash as Trash } from "@tabler/icons-react";
import { deleteSharedMemoryCard, exportSharedMemoryCard, getSharedMemoryCard, listSharedArtifacts, updateSharedMemoryCard } from "../lib/sharedApi";
import { type ArtifactSummary, type MemoryCardDetail, type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { Button } from "../components/ui/button";
import { PageHeader } from "../components/ui/page-header";
import { Combobox } from "../components/ui/combobox";
import { useConfirm } from "../components/ui/confirm-dialog";
import { EmptyState } from "../components/ui/empty-state";
import { Field } from "../components/ui/field";
import { Input } from "../components/ui/input";
import { LoadingState } from "../components/ui/loading-state";
import { Textarea } from "../components/ui/textarea";
import { Toast } from "../components/ui/toast";
import { SharedLayout } from "../components/SharedLayout";
import { invalidateKeys, queryClient, workspaceKeys } from "../lib/queryClient";
import { useLiveEventHub, useLiveEvents } from "../lib/liveEvents";
import { useServerState } from "../lib/useServerState";
import { useUnsavedChanges } from "../lib/unsavedChanges";
import { LEDGER_ACTIONS, LIVE_REFRESH_DELAY_MS, type WorkspaceSection } from "./constants";
import { apiMessage, citationLines } from "./format";
import { navigate } from "./router";
import { OrganizationRail, WorkspaceTopbar } from "./Organization";
import { ICON_SIZE } from "../components/ui/icon";
import { submitHint } from "../lib/submitShortcut";

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
  const confirm = useConfirm();
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
  // The evidence the card cites now, which editing starts from.
  const citedEvidence: EditableCitation[] = (card?.evidence ?? []).filter((entry) => entry.artifact_id && entry.exists).map((entry) => ({
    artifact_id: entry.artifact_id!,
    chunk_id: entry.chunk_id,
    title: entry.title ?? "Untitled evidence",
    path: entry.path ?? "",
    start_line: entry.start_line,
    end_line: entry.end_line,
    confidence: null,
  }));
  const citationKey = (list: EditableCitation[]) => list.map((entry) => `${entry.artifact_id}:${entry.chunk_id ?? ""}`).join("|");
  const isDirty = isEditing && Boolean(card) && (
    title !== card!.card.title || bodyMarkdown !== card!.card.body_markdown || source !== card!.card.source || citationKey(citations) !== citationKey(citedEvidence)
  );
  const canLeave = useUnsavedChanges(isDirty, { subject: "your edits to this memory card" });
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
    setCitations(citedEvidence);
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
    if (!(await confirm({ action: "Delete", danger: true, description: "This cannot be undone.", title: `Delete ${card?.card.title ?? "this memory card"}?` }))) return;
    setIsMutating(true); setError(null);
    try { await deleteSharedMemoryCard(accessToken, cardId); onBack(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  return (
    <SharedRecordLayout activeSection="memory" apiAvailable={apiAvailable} organization={organization} organizations={organizations} session={session} signOut={signOut} title={card?.card.title ?? "Memory card"} subtitle={card ? `Source: ${card.card.source}` : "Loading durable team memory…"} workspace={workspace} workspaces={workspaces}>
      <div className="shared-detail-actions">{canWrite ? <><Button disabled={isLoading || isMutating} onClick={() => { if (!isEditing) startEditing(); else void canLeave().then((leave) => { if (leave) setIsEditing(false); }); }} variant="secondary"><Pencil size={ICON_SIZE.md} /> {isEditing ? "Cancel edit" : "Edit"}</Button><Button className="shared-danger-action" disabled={isLoading || isMutating} onClick={() => void removeCard()} variant="secondary"><Trash size={ICON_SIZE.md} /> Delete</Button></> : null}<Button busy={isExporting} disabled={isLoading || isMutating} onClick={() => void exportCard()} variant="main"><FileText size={ICON_SIZE.md} /> Export Markdown</Button></div>
      <Toast kind="error" message={error} />
      {!card && isLoading ? <LoadingState label="Loading the memory card" /> : null}
      {card?.review.length ? <section className="shared-record-panel shared-review-panel"><h2>Needs review</h2><p className="shared-muted-copy">Evidence this card cites has changed since it was written. Check what it says, edit it or its evidence, or confirm it as it is.</p><ul>{card.review.map((reason, index) => <li key={`${reason.kind}-${index}`}>{reason.detail}{reason.artifact_id ? <> <button className="shared-assistant-citation" onClick={() => onOpenArtifact(reason.artifact_id!)} type="button">Open</button></> : null}</li>)}</ul>{canWrite ? <Button busy={isMutating} onClick={() => void confirmCard()} variant="secondary">Confirm as it is</Button> : null}</section> : null}
      <section className="shared-record-panel"><h2>Durable statement</h2>{isEditing ? <form className="shared-record-edit-form" onSubmit={saveCard}><Field label="Title" required><Input onChange={(event) => setTitle(event.target.value)} value={title} /></Field><Field label="Source" required><Input onChange={(event) => setSource(event.target.value)} value={source} /></Field><Field hint={submitHint("saves")} label="Statement" required><Textarea onChange={(event) => setBodyMarkdown(event.target.value)} value={bodyMarkdown} /></Field><fieldset className="shared-memory-citations"><legend>Evidence this card cites</legend>{citations.length ? <ul>{citations.map((citation, index) => <li key={`${citation.artifact_id}-${citation.chunk_id ?? "file"}-${index}`}><span>{citation.title}{citation.chunk_id ? citationLines(citation) : " (whole file)"}</span><Button onClick={() => setCitations((current) => current.filter((_, position) => position !== index))} variant="secondary">Remove</Button></li>)}</ul> : <EmptyState title="No evidence is cited" />}<div className="shared-memory-citation-add"><Field label="Evidence to add">{(control) => <Combobox aria-label="Evidence to add" emptyLabel="No file matches" id={control.id} onValueChange={setAddArtifactId} options={artifacts.filter((artifact) => !artifact.repository_id).map((artifact) => ({ label: artifact.title, value: artifact.id }))} placeholder="Add a file…" value={addArtifactId} />}</Field><Button disabled={!addArtifactId} onClick={addCitation} variant="secondary">Add</Button></div></fieldset><Button busy={isMutating} type="submit" variant="main"><Pencil size={ICON_SIZE.md} /> Save memory</Button></form> : <div className="shared-markdown shared-memory-body"><SafeMarkdown>{card?.card.body_markdown ?? ""}</SafeMarkdown></div>}</section>
      <section className="shared-record-panel"><h2>Linked evidence</h2>{card?.evidence.length ? <div className="shared-evidence-links">{card.evidence.map((evidence) => <article key={evidence.link_id}>{evidence.artifact_id && evidence.exists ? <button className="shared-assistant-citation shared-evidence-link-title" onClick={() => onOpenArtifact(evidence.artifact_id!)} type="button"><strong>{evidence.title ?? "Untitled evidence"}</strong></button> : <strong>{evidence.title ?? "Untitled evidence"}{evidence.exists ? "" : " (missing)"}</strong>}<span>{evidence.path ?? evidence.target_id}{citationLines(evidence)}</span></article>)}</div> : <EmptyState title="This memory card currently has no linked evidence" />}</section>
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
        <PageHeader description={subtitle} title={title} />
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
        <Button className="shared-route-action" onClick={onBack} type="button" variant="main"><ArrowLeft size={ICON_SIZE.md} /> Go to workspaces</Button>
      </section>
    </SharedLayout>
  );
}
