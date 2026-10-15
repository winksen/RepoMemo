import { type FormEvent, useEffect, useRef, useState } from "react";
import { SafeMarkdown } from "../components/SafeMarkdown";
import { IconLoader2 as Loader, IconMessageCircle as MessageCircle, IconPencil as Pencil, IconShieldLock as Shield, IconStack2 as Layers, IconTrash as Trash, IconUpload as Upload } from "@tabler/icons-react";
import { uploadSharedArtifactVersion, createSharedArtifactComment, deleteSharedArtifact, deleteSharedArtifactComment, getSharedArtifact, getSharedArtifactLifecycle, indexSharedArtifact, listSharedArtifactChunks, waitForSharedJob, listSharedArtifactComments, listSharedArtifactLifecycleEvents, updateSharedArtifact, updateSharedArtifactComment, updateSharedArtifactLifecycle } from "../lib/sharedApi";
import { type ArtifactComment, type ArtifactDetail, type ArtifactLifecycle, type ArtifactLifecycleEvent, type ArtifactLifecycleStatus, type ArtifactSummary, type Chunk, type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { Button } from "../components/ui/button";
import { Dropdown } from "../components/ui/dropdown";
import { Input } from "../components/ui/input";
import { Textarea } from "../components/ui/textarea";
import { ImagePreview } from "../components/ImagePreview";
import { isDocumentArtifact } from "../lib/documents";
import { DocumentViewer, RepositoryDetailView } from "./lazy";
import { Toast, showToast } from "../components/ui/toast";
import { LiveEventsContext, useLiveEventHub, useLiveEvents } from "../lib/liveEvents";
import { invalidateKeys, queryClient, workspaceKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import { INDEX_POLL_ATTEMPTS, INDEX_POLL_INTERVAL_MS, LIVE_FALLBACK_POLL_MS } from "./constants";
import { useArtifacts, useMembers } from "./workspace/data";
import { apiMessage, formatActivityTime, lastChangeLabel } from "./format";
import { ArtifactMentionsPanel } from "./Mentions";
import { SharedRecordLayout } from "./MemoryCardDetail";

export function SharedArtifactDetail({
  accessToken,
  apiAvailable,
  artifactId,
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
  artifactId: string;
  onBack: () => void;
  onOpenArtifact: (artifactId: string) => void;
  organization?: Organization;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspace: SharedWorkspace;
  workspaces: SharedWorkspace[];
}) {
  // Everything the page shows is in the query cache, under its workspace: shown
  // at once on a later visit, refetched when the page opens, and marked stale
  // by live events and by changes made here.
  const workspaceId = workspace.workspace.id;
  const [artifact, setArtifact, artifactQuery] = useServerState<ArtifactDetail | null>({
    key: workspaceKeys.artifact(workspaceId, artifactId), fetch: () => getSharedArtifact(accessToken, artifactId), initial: null, staleTime: 0,
  });
  const [comments, setComments, commentsQuery] = useServerState<ArtifactComment[]>({
    key: workspaceKeys.artifactComments(workspaceId, artifactId), fetch: () => listSharedArtifactComments(accessToken, artifactId), initial: [], staleTime: 0,
  });
  const [lifecycle, , lifecycleQuery] = useServerState<ArtifactLifecycle | null>({
    key: workspaceKeys.artifactLifecycle(workspaceId, artifactId), fetch: () => getSharedArtifactLifecycle(accessToken, artifactId), initial: null, staleTime: 0,
  });
  const [lifecycleEvents, , lifecycleEventsQuery] = useServerState<ArtifactLifecycleEvent[]>({
    key: workspaceKeys.artifactLifecycleEvents(workspaceId, artifactId), fetch: () => listSharedArtifactLifecycleEvents(accessToken, artifactId), initial: [], staleTime: 0,
  });
  const [workspaceMembers, , membersQuery] = useMembers(accessToken, workspaceId, true);
  const [workspaceArtifacts, , workspaceArtifactsQuery] = useArtifacts(accessToken, workspaceId);
  const pageQueries = [artifactQuery, commentsQuery, lifecycleQuery, lifecycleEventsQuery, membersQuery, workspaceArtifactsQuery];
  const [isReloading, setIsReloading] = useState(false);
  const isLoading = isReloading || pageQueries.some((query) => query.isLoading);
  const loadError = pageQueries.find((query) => query.error)?.error ?? null;
  const [commentBody, setCommentBody] = useState("");
  const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
  const [editingCommentBody, setEditingCommentBody] = useState("");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { if (loadError) setError(apiMessage(loadError)); }, [loadError]);
  const [showChunks, setShowChunks] = useState(false);
  const [indexingStalled, setIndexingStalled] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isEditingLifecycle, setIsEditingLifecycle] = useState(false);
  const [isMutating, setIsMutating] = useState(false);
  const [title, setTitle] = useState("");
  const [lifecycleStatus, setLifecycleStatus] = useState<ArtifactLifecycleStatus>("active");
  const [lifecycleOwnerId, setLifecycleOwnerId] = useState("");
  const [lifecycleNote, setLifecycleNote] = useState("");
  const [supersededByArtifactId, setSupersededByArtifactId] = useState("");
  const canWrite = workspace.role !== "viewer";
  const canModerateComments = workspace.role === "owner" || workspace.role === "admin";
  const canInspectIndex = workspace.role === "owner" || workspace.role === "admin";
  // Repository files, and the item that stands for a whole repository,
  // change through their repository's syncs only.
  const isRepositoryItem = artifact?.summary.artifact_type === "repository";
  const isRepositoryFile = artifact?.metadata?.origin === "git_repo" || isRepositoryItem;
  const isAwaitingIndex = Boolean(artifact) && !artifact?.summary.indexed_at;

  const versionInput = useRef<HTMLInputElement | null>(null);

  /** The file replaces this artifact's content; comments, folder, state and memory links stay. */
  async function uploadVersion(file: File) {
    setIsMutating(true); setError(null);
    try {
      await uploadSharedArtifactVersion(accessToken, artifactId, file);
      showToast("success", "New version stored. It is being indexed again.");
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  /** Marks everything this page shows stale and waits for the refetch (after a change that touches several parts). */
  async function load() {
    setIsReloading(true); setError(null);
    try {
      await invalidateKeys([
        workspaceKeys.artifact(workspaceId, artifactId),
        workspaceKeys.artifactComments(workspaceId, artifactId),
        workspaceKeys.artifactLifecycle(workspaceId, artifactId),
        workspaceKeys.artifactLifecycleEvents(workspaceId, artifactId),
        workspaceKeys.members(workspaceId),
        workspaceKeys.artifacts(workspaceId),
      ]);
    } finally { setIsReloading(false); }
  }

  useEffect(() => { setTitle(artifact?.summary.title ?? ""); }, [artifact?.summary.title]);
  useEffect(() => {
    setLifecycleStatus(lifecycle?.status ?? "active");
    setLifecycleOwnerId(lifecycle?.owner?.id ?? "");
    setLifecycleNote(lifecycle?.review_note ?? "");
    setSupersededByArtifactId(lifecycle?.superseded_by_artifact_id ?? "");
  }, [lifecycle]);

  // Live updates: indexing finishing, and other people's comments and
  // reviews of this file, arrive on the workspace event stream. The
  // repository page below listens to the same stream for sync progress.
  const liveHub = useLiveEventHub(`/v1/workspaces/${encodeURIComponent(workspace.workspace.id)}/events`, accessToken);
  const isLive = useLiveEvents((event) => {
    if (event.type === "resync") { void load(); return; }
    if (event.type !== "activity" || event.event.subject_id !== artifactId) return;
    const { action, actor } = event.event;
    const byOthers = actor?.id !== session.user.id;
    if (action === "artifact_indexed" || action === "artifact_updated") {
      void invalidateKeys([workspaceKeys.artifact(workspaceId, artifactId)]);
    } else if (action.startsWith("comment_") && byOthers) {
      void invalidateKeys([workspaceKeys.artifactComments(workspaceId, artifactId)]);
    } else if (action === "evidence_lifecycle_updated" && byOthers) {
      void invalidateKeys([workspaceKeys.artifactLifecycle(workspaceId, artifactId), workspaceKeys.artifactLifecycleEvents(workspaceId, artifactId)]);
    }
  }, liveHub);

  // Indexing happens in the background right after the evidence is stored.
  // With the live stream open, the "indexed" event ends the wait and this
  // poll is only a safety net.
  useEffect(() => {
    setIndexingStalled(false);
    if (!isAwaitingIndex) return;
    const interval = isLive ? LIVE_FALLBACK_POLL_MS : INDEX_POLL_INTERVAL_MS;
    const maxAttempts = Math.ceil((INDEX_POLL_ATTEMPTS * INDEX_POLL_INTERVAL_MS) / interval);
    let attempts = 0;
    const timer = window.setInterval(async () => {
      attempts += 1;
      try {
        const nextArtifact = await getSharedArtifact(accessToken, artifactId);
        setArtifact(nextArtifact);
        if (nextArtifact.summary.indexed_at) { window.clearInterval(timer); return; }
      } catch { /* keep the last known state and try again */ }
      if (attempts >= maxAttempts) {
        window.clearInterval(timer);
        setIndexingStalled(true);
      }
    }, interval);
    return () => window.clearInterval(timer);
  }, [accessToken, artifactId, isAwaitingIndex, isLive]);

  async function saveArtifact(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsMutating(true); setError(null);
    try { await updateSharedArtifact(accessToken, artifactId, title); setIsEditing(false); await load(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function removeArtifact() {
    if (!window.confirm(`Delete ${artifact?.summary.title ?? "this evidence"}? Linked memory citations will be removed.`)) return;
    setIsMutating(true); setError(null);
    try { await deleteSharedArtifact(accessToken, artifactId); onBack(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function saveLifecycle(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsMutating(true); setError(null);
    try {
      await updateSharedArtifactLifecycle(accessToken, artifactId, {
        status: lifecycleStatus,
        ownerUserId: lifecycleOwnerId || null,
        reviewNote: lifecycleNote,
        supersededByArtifactId: supersededByArtifactId || null,
      });
      setIsEditingLifecycle(false);
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function addComment(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsMutating(true); setError(null);
    try {
      const comment = await createSharedArtifactComment(accessToken, artifactId, commentBody);
      setComments((current) => [...current, comment]);
      setCommentBody("");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function saveComment(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingCommentId) return;
    setIsMutating(true); setError(null);
    try {
      const updated = await updateSharedArtifactComment(accessToken, editingCommentId, editingCommentBody);
      setComments((current) => current.map((comment) => comment.id === updated.id ? updated : comment));
      setEditingCommentId(null); setEditingCommentBody("");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function removeComment(comment: ArtifactComment) {
    if (!window.confirm("Delete this evidence comment?")) return;
    setIsMutating(true); setError(null);
    try {
      await deleteSharedArtifactComment(accessToken, comment.id);
      setComments((current) => current.filter((entry) => entry.id !== comment.id));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  return (
    <LiveEventsContext.Provider value={liveHub}>
    <SharedRecordLayout activeSection="evidence" apiAvailable={apiAvailable} organization={organization} organizations={organizations} session={session} signOut={signOut} title={artifact?.summary.title ?? "Artifact"} subtitle={artifact?.summary.path ?? "Loading protected evidence…"} workspace={workspace} workspaces={workspaces}>
      <div className="shared-detail-actions">{isRepositoryFile ? <span className="shared-muted-copy">{isRepositoryItem ? "Kept in step with the repository, last synced at commit" : "Synced from a repository at commit"} <code>{String(artifact?.metadata?.commit ?? "").slice(0, 7)}</code>{artifact?.metadata?.last_author ? <> · last changed by {lastChangeLabel(String(artifact.metadata.last_author), String(artifact.metadata.last_changed_at ?? ""))}{artifact.metadata.last_change_commit ? <> in <code>{String(artifact.metadata.last_change_commit).slice(0, 7)}</code></> : null}</> : null}{Array.isArray(artifact?.metadata?.code_owners) && artifact.metadata.code_owners.length ? <> · CODEOWNERS suggests {(artifact.metadata.code_owners as unknown[]).map(String).join(", ")}</> : null}</span> : canWrite ? <><Button disabled={isLoading || isMutating} onClick={() => setIsEditing((value) => !value)} type="button" variant="secondary"><Pencil size={16} /> {isEditing ? "Cancel edit" : "Edit"}</Button><Button className="shared-danger-action" disabled={isLoading || isMutating} onClick={() => void removeArtifact()} type="button" variant="secondary"><Trash size={16} /> Delete</Button></> : null}{canWrite && !isRepositoryFile ? <><input accept={artifact ? `.${artifact.summary.path.split(".").pop() ?? ""}` : undefined} aria-label="New version of this file" hidden onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void uploadVersion(file); }} ref={versionInput} type="file" /><Button disabled={isLoading || isMutating || !artifact} onClick={() => versionInput.current?.click()} type="button" variant="secondary"><Upload size={16} /> New version</Button></> : null}{canInspectIndex ? <Button disabled={isLoading || !artifact} onClick={() => setShowChunks(true)} type="button" variant="secondary"><Layers size={16} /> Indexed chunks</Button> : null}</div>
      <Toast kind="error" message={error} />
      <div className="shared-record-meta"><span>{artifact?.summary.artifact_type ?? "artifact"}</span><span>{artifact?.summary.language ?? "Unspecified language"}</span><span>{artifact?.summary.indexed_at ? "Indexed" : indexingStalled ? "Not indexed" : "Indexing…"}</span></div>
      {isEditing ? <form className="shared-record-edit-form" onSubmit={saveArtifact}><label>Evidence title<Input onChange={(event) => setTitle(event.target.value)} required value={title} /></label><Button disabled={isMutating} type="submit" variant="main">{isMutating ? <Loader className="spin" size={16} /> : <Pencil size={16} />} Save evidence</Button></form> : null}
      {artifact && (isRepositoryFile || artifact.summary.artifact_type === "issue" || artifact.summary.artifact_type === "pr") ? <ArtifactMentionsPanel accessToken={accessToken} artifactId={artifactId} onOpenArtifact={onOpenArtifact} /> : null}
      <section className="shared-record-panel shared-lifecycle-panel"><div className="shared-panel-heading"><div><Shield size={18} /><h2>Evidence lifecycle</h2></div><span className={`shared-lifecycle-status status-${lifecycle?.status ?? "active"}`}>{lifecycle?.status?.replace(/_/g, " ") ?? "Active"}</span></div>{isEditingLifecycle ? <form className="shared-lifecycle-form" onSubmit={saveLifecycle}><label>Status<Dropdown aria-label="Evidence lifecycle status" onValueChange={(value) => setLifecycleStatus(value as ArtifactLifecycleStatus)} options={[{ label: "Active", value: "active" }, { label: "Needs review", value: "needs_review" }, { label: "Verified", value: "verified" }, { label: "Outdated", value: "outdated" }, { label: "Superseded", value: "superseded" }]} value={lifecycleStatus} /></label><label>Review owner<Dropdown aria-label="Evidence review owner" onValueChange={(value) => setLifecycleOwnerId(value === "__unassigned__" ? "" : value)} options={[{ label: "Unassigned", value: "__unassigned__" }, ...workspaceMembers.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={lifecycleOwnerId || "__unassigned__"} /></label>{lifecycleStatus === "superseded" ? <label>Replacement evidence<Dropdown aria-label="Replacement evidence" onValueChange={(value) => setSupersededByArtifactId(value === "__none__" ? "" : value)} options={[{ label: "Choose replacement evidence", value: "__none__" }, ...workspaceArtifacts.filter((entry) => entry.id !== artifactId).map((entry) => ({ label: entry.title, value: entry.id }))]} value={supersededByArtifactId || "__none__"} /></label> : null}<label className="shared-lifecycle-note">Review note<Textarea onChange={(event) => setLifecycleNote(event.target.value)} placeholder="What should the team know about this evidence?" value={lifecycleNote} /></label><div><Button disabled={isMutating} type="submit" variant="main">{isMutating ? <Loader className="spin" size={16} /> : <Shield size={16} />} Save lifecycle</Button><Button disabled={isMutating} onClick={() => setIsEditingLifecycle(false)} type="button" variant="secondary">Cancel</Button></div></form> : <div className="shared-lifecycle-summary"><div><strong>{lifecycle?.owner ? `Owned by ${lifecycle.owner.display_name}` : "No review owner"}</strong><span>{lifecycle?.reviewed_at ? `Last updated ${formatActivityTime(lifecycle.reviewed_at)} by ${lifecycle.reviewed_by?.display_name ?? "a member"}` : "No lifecycle review recorded yet."}</span></div>{lifecycle?.review_note ? <p>{lifecycle.review_note}</p> : <p className="shared-muted-copy">Add a review note so the evidence can be trusted in context.</p>}{lifecycle?.superseded_by_artifact_id ? <span className="shared-lifecycle-replacement">Replaced by evidence {lifecycle.superseded_by_artifact_id.slice(0, 8)}</span> : null}{canWrite ? <Button onClick={() => setIsEditingLifecycle(true)} type="button" variant="secondary"><Pencil size={15} /> Update lifecycle</Button> : null}</div>}<div className="shared-lifecycle-history"><strong>History</strong>{lifecycleEvents.length ? lifecycleEvents.map((event) => <p key={event.id}><span>{event.detail}</span><time dateTime={event.created_at}>{event.actor?.display_name ?? "System"} · {formatActivityTime(event.created_at)}</time></p>) : <p className="shared-muted-copy">Lifecycle changes will be recorded here.</p>}</div></section>
      {artifact?.summary.artifact_type === "image" ? <section className="shared-record-panel"><h2>Image</h2><ImagePreview accessToken={accessToken} artifactId={artifact.summary.id} key={artifact.summary.id} title={artifact.summary.title} /></section> : null}
      {artifact && isRepositoryItem ? <RepositoryDetailView accessToken={accessToken} canSync={canWrite} key={artifact.summary.source_id} onOpenArtifact={onOpenArtifact} repositoryId={artifact.summary.source_id} /> : artifact && isDocumentArtifact(artifact.summary) ? <section className="shared-record-panel"><h2>Document</h2><DocumentViewer accessToken={accessToken} artifact={artifact.summary} key={artifact.summary.id} /></section> : <section className="shared-record-panel"><h2>Stored content</h2>{artifact?.content_preview ? artifact.summary.artifact_type === "note" ? <div className="shared-content-preview shared-markdown"><SafeMarkdown>{artifact.content_preview}</SafeMarkdown></div> : <pre className="shared-content-preview">{artifact.content_preview}</pre> : <p className="shared-muted-copy">This artifact has no text preview available.</p>}</section>}
      <section className="shared-record-panel shared-discussion-panel"><div className="shared-panel-heading"><div><MessageCircle size={18} /><h2>Evidence discussion</h2></div><span>{comments.length} comments</span></div>{comments.length ? <div className="shared-comment-list">{comments.map((comment) => <article key={comment.id}><div className="shared-comment-author"><span aria-hidden="true">{comment.author.display_name.slice(0, 1).toUpperCase()}</span><div><strong>{comment.author.display_name}</strong><time dateTime={comment.created_at}>{formatActivityTime(comment.created_at)}{comment.updated_at !== comment.created_at ? " · edited" : ""}</time></div></div>{editingCommentId === comment.id ? <form onSubmit={saveComment}><Textarea aria-label="Edit comment" onChange={(event) => setEditingCommentBody(event.target.value)} required value={editingCommentBody} /><div><Button disabled={isMutating} type="submit" variant="main">Save comment</Button><Button onClick={() => setEditingCommentId(null)} type="button" variant="secondary">Cancel</Button></div></form> : <><p>{comment.body}</p>{comment.author.id === session.user.id || canModerateComments ? <div className="shared-comment-actions">{comment.author.id === session.user.id ? <Button onClick={() => { setEditingCommentId(comment.id); setEditingCommentBody(comment.body); }} type="button" variant="secondary"><Pencil size={14} /> Edit</Button> : null}<Button disabled={isMutating} onClick={() => void removeComment(comment)} type="button" variant="secondary"><Trash size={14} /> Delete</Button></div> : null}</>}</article>)}</div> : <p className="shared-muted-copy">No discussion yet. Add context, ask for a review, or record a decision beside the evidence.</p>}{canWrite ? <form className="shared-comment-form" onSubmit={addComment}><Textarea aria-label="New evidence comment" onChange={(event) => setCommentBody(event.target.value)} placeholder="Add context or mention @teammate@example.com…" required value={commentBody} /><Button disabled={isMutating} type="submit" variant="main"><MessageCircle size={16} /> Add comment</Button></form> : null}</section>
      {showChunks && artifact ? <IndexedChunksDialog accessToken={accessToken} artifact={artifact.summary} onClose={() => setShowChunks(false)} onReindexed={() => void load()} /> : null}
    </SharedRecordLayout>
    </LiveEventsContext.Provider>
  );
}

/** Administrator-only inspection of the chunks stored for one artifact. */
export function IndexedChunksDialog({
  accessToken,
  artifact,
  onClose,
  onReindexed,
}: {
  accessToken: string;
  artifact: ArtifactSummary;
  onClose: () => void;
  onReindexed: () => void;
}) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const chunksKey = workspaceKeys.artifactChunks(artifact.workspace_id, artifact.id);
  const [chunks, , chunksQuery] = useServerState<Chunk[] | null>({
    key: chunksKey, fetch: () => listSharedArtifactChunks(accessToken, artifact.id), initial: null, staleTime: 0,
  });
  const [reindexError, setError] = useState<string | null>(null);
  const error = reindexError ?? (chunksQuery.error ? apiMessage(chunksQuery.error) : null);
  const [isReindexing, setIsReindexing] = useState(false);
  const mounted = useRef(true);
  useEffect(() => () => { mounted.current = false; }, []);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (dialog && !dialog.open) dialog.showModal();
  }, [artifact.id]);

  async function reindex() {
    setIsReindexing(true); setError(null);
    try {
      // The server accepts the re-index and does it in the background: follow its job.
      const job = await waitForSharedJob(accessToken, await indexSharedArtifact(accessToken, artifact.id), () => !mounted.current);
      if (!mounted.current) return;
      if (job.status !== "completed") throw new Error(job.error_message ?? "Indexing did not finish.");
      await queryClient.invalidateQueries({ queryKey: chunksKey });
      onReindexed();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsReindexing(false); }
  }

  return (
    <dialog aria-labelledby="indexed-chunks-title" className="shared-dialog" onClick={(event) => { if (event.target === event.currentTarget) dialogRef.current?.close(); }} onClose={onClose} ref={dialogRef}>
      <header className="shared-dialog-header">
        <div><h2 id="indexed-chunks-title">Indexed chunks</h2><p>{artifact.path}{chunks ? ` · ${chunks.length} ${chunks.length === 1 ? "chunk" : "chunks"}` : ""}</p></div>
        <Button aria-label="Close indexed chunks" onClick={() => dialogRef.current?.close()} type="button" variant="secondary">Close</Button>
      </header>
      <div className="shared-dialog-body">
        <Toast kind="error" message={error} />
        {chunks === null && !error ? <p className="shared-muted-copy"><Loader className="spin" size={14} /> Loading chunks…</p> : null}
        {chunks && chunks.length === 0 ? <p className="shared-muted-copy">No chunks are stored for this artifact. It may still be indexing, or it has no extractable text.</p> : null}
        {chunks && chunks.length ? <div className="shared-chunk-list">{chunks.map((chunk) => <article key={chunk.id}><span>{`#${chunk.chunk_index + 1}`}{chunk.start_line ? ` · Lines ${chunk.start_line}${chunk.end_line && chunk.end_line !== chunk.start_line ? `–${chunk.end_line}` : ""}` : ""}{chunk.heading_path ? ` · ${chunk.heading_path}` : ""}{chunk.token_count ? ` · ${chunk.token_count} tokens` : ""}</span><p>{chunk.text}</p></article>)}</div> : null}
      </div>
      <footer className="shared-dialog-footer">
        <span>Only workspace owners and administrators can see stored chunks.</span>
        <Button disabled={isReindexing} onClick={() => void reindex()} type="button" variant="main">{isReindexing ? <Loader className="spin" size={16} /> : <Layers size={16} />} Re-index</Button>
      </footer>
    </dialog>
  );
}
