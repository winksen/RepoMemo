import { useEffect, useMemo, useRef, useState } from "react";
import { IconAlertCircle as AlertCircle, IconLoader2 as Loader, IconSearch as Search, IconSettings as Settings, IconShieldLock as Shield, IconUpload as Upload } from "@tabler/icons-react";
import { cancelSharedJob, createSharedMemoryCard, getSharedRetrievalFacets, getSharedWorkspaceMetrics, getSharedWorkspaceOverview, listSharedArtifacts, listSharedIndexFailures, listSharedMemoryCards } from "../lib/sharedApi";
import { type Citation, type IndexingJobStatus, type LiveEvent, type Organization, type SharedSession, type SharedWorkspace, type WorkspaceActivityEvent } from "../types";
import { Button } from "../components/ui/button";
import { PageHeader } from "../components/ui/page-header";
import { Checkbox } from "../components/ui/checkbox";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { Field } from "../components/ui/field";
import { Input } from "../components/ui/input";
import { Textarea } from "../components/ui/textarea";
import { AssistantDock } from "../components/AssistantDock";
import { KnowledgeMapPanel, RepositoriesPanel, ReviewQueuePanel, WorkspaceHealthPanel } from "./lazy";
import { Toast, showToast } from "../components/ui/toast";
import { LiveEventsContext, useLiveEventHub, useLiveEvents } from "../lib/liveEvents";
import { SharedLayout } from "../components/SharedLayout";
import { ADMIN_WORKSPACE_SECTIONS, INDEX_POLL_ATTEMPTS, INDEX_POLL_INTERVAL_MS, JOB_KIND_LABEL, JOB_STAGE_LABEL, LEDGER_ACTIONS, LIVE_FALLBACK_POLL_MS, LIVE_METRICS_DELAY_MS, LIVE_REFRESH_DELAY_MS, type LiveRefresh, MEMORY_ACTIONS, type MemoryDraft, NO_LIVE_REFRESH, TASK_ACTIONS, type WorkspaceSection } from "./constants";
import { apiMessage, citationLines } from "./format";
import { navigate } from "./router";
import { useUnsavedChanges } from "../lib/unsavedChanges";
import { invalidateKeys, queryClient, workspaceInvalidation, workspaceKeys } from "../lib/queryClient";
import { OrganizationRail, WorkspaceTopbar } from "./Organization";
import { EvidenceSection } from "./workspace/EvidenceSection";
import { RetrievalSection } from "./workspace/RetrievalSection";
import { MemorySection } from "./workspace/MemorySection";
import { TasksSection } from "./workspace/TasksSection";
import { PeopleSection } from "./workspace/PeopleSection";
import { SettingsSection } from "./workspace/SettingsSection";
import { OverviewSection } from "./workspace/OverviewSection";
import { useAiProviders, useArtifacts, useCapabilities, useFolders, useIndexFailures, useMembers, useMemoryCards, useMetrics, useOverview, useRetrievalFacets, useRunningJobs } from "./workspace/data";
import { type WorkspacePage } from "./workspace/page";
import { ICON_SIZE } from "../components/ui/icon";
import { submitHint } from "../lib/submitShortcut";

export function SharedWorkspaceDetail({
  accessToken,
  apiAvailable,
  onWorkspaceDeleted,
  onWorkspaceUpdated,
  onOpenArtifact,
  onOpenMemoryCard,
  onNavigate,
  organization,
  organizations,
  section,
  session,
  signOut,
  workspace,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  onWorkspaceDeleted: (workspaceId: string) => void;
  onWorkspaceUpdated: (workspace: SharedWorkspace["workspace"]) => void;
  onOpenArtifact: (artifactId: string) => void;
  onOpenMemoryCard: (cardId: string) => void;
  onNavigate: (section: WorkspaceSection) => void;
  organization?: Organization;
  organizations: Organization[];
  section: WorkspaceSection;
  session: SharedSession;
  signOut: () => void;
  workspace: SharedWorkspace;
  workspaces: SharedWorkspace[];
}) {
  const [isReloading, setIsReloading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [liveJobs, setLiveJobs] = useState<Record<string, IndexingJobStatus>>({});
  const liveRefresh = useRef<LiveRefresh>({ ...NO_LIVE_REFRESH });
  const [indexingStalled, setIndexingStalled] = useState(false);
  const [memoryDraft, setMemoryDraft] = useState<MemoryDraft | null>(null);
  const memoryDraftOrigin = useRef({ title: "", body: "" });
  const canLeaveMemoryDraft = useUnsavedChanges(Boolean(memoryDraft) && (memoryDraft!.title !== memoryDraftOrigin.current.title || memoryDraft!.body !== memoryDraftOrigin.current.body), { subject: "your edits to the memory card" });
  const closeMemoryDraft = () => { void canLeaveMemoryDraft().then((leave) => { if (leave) setMemoryDraft(null); }); };
  // Server state lives in the query cache (see lib/queryClient): it is shared
  // with other pages, refreshed in the background, and marked stale by live
  // events and mutations instead of the page reloading everything. Each section
  // reads its own data through the hooks of workspace/data.ts.
  const workspaceId = workspace.workspace.id;
  // What the sections remember between visits; a different workspace starts clean.
  const store = useMemo(() => new Map<string, unknown>(), [workspaceId]);
  const [capabilities, , capabilitiesQuery] = useCapabilities(accessToken, workspaceId);
  const [, setOverview, overviewQuery] = useOverview(accessToken, workspaceId);
  const [, setWorkspaceMetrics, metricsQuery] = useMetrics(accessToken, workspaceId);
  const [artifacts, setArtifacts, artifactsQuery] = useArtifacts(accessToken, workspaceId);
  const [indexFailures, setIndexFailures] = useIndexFailures(accessToken, workspaceId);
  const [, , foldersQuery] = useFolders(accessToken, workspaceId);
  const [, setMemoryCards, memoryCardsQuery] = useMemoryCards(accessToken, workspaceId);
  const [, setRetrievalFacets, facetsQuery] = useRetrievalFacets(accessToken, workspaceId);
  const [, , runningJobsQuery] = useRunningJobs(accessToken, workspaceId);
  const canSeeMembers = capabilities?.can_manage_members ?? false;
  // Read ahead for the sections that need them, as the page always did.
  useMembers(accessToken, workspaceId, canSeeMembers);
  useAiProviders(accessToken, workspaceId, canSeeMembers);

  const coreQueries = [capabilitiesQuery, overviewQuery, metricsQuery, artifactsQuery, foldersQuery, memoryCardsQuery, facetsQuery];
  // Only the first load, or a reload someone asked for, counts as loading; background refreshes stay quiet.
  const isLoading = isReloading || coreQueries.some((query) => query.isLoading);
  const loadError = coreQueries.find((query) => query.error)?.error ?? null;
  useEffect(() => { if (loadError) setError(apiMessage(loadError)); }, [loadError]);
  // Running jobs seed the live progress panel once per fetch.
  useEffect(() => {
    if (runningJobsQuery.data) setLiveJobs(Object.fromEntries(runningJobsQuery.data.map((job) => [job.id, job])));
  }, [runningJobsQuery.dataUpdatedAt]);
  const canWrite = capabilities?.can_write_content ?? false;
  const canManageMembers = capabilities?.can_manage_members ?? false;
  const canAssignAdmin = capabilities?.can_assign_admin ?? false;
  const canManageWorkspace = capabilities?.can_manage_workspace ?? false;
  const canInspectIndex = capabilities?.can_inspect_index ?? false;
  const pendingIndexKey = useMemo(() => artifacts.filter((artifact) => !artifact.indexed_at && !indexFailures[artifact.id]).map((artifact) => artifact.id).join(","), [artifacts, indexFailures]);
  const isEvidenceView = section === "evidence";
  const isDocumentsView = section === "documents";
  const isRepositoriesView = section === "repositories";
  const isRetrievalView = section === "retrieval";
  const isMapView = section === "map";
  const isHealthView = section === "health";
  const isReviewView = section === "review";
  const isMemoryView = section === "memory";
  const isTasksView = section === "tasks";
  const isPeopleView = section === "people";
  const isActivityView = isPeopleView;
  const isSettingsView = section === "settings";
  const isWorkspaceAdmin = workspace.role === "owner" || workspace.role === "admin";
  const isRestrictedView = ADMIN_WORKSPACE_SECTIONS.has(section) && !isWorkspaceAdmin;

  /** Everything this page shows is refetched (after a change that touches many things). */
  async function load() {
    setIsReloading(true);
    setError(null);
    try {
      await queryClient.invalidateQueries({ queryKey: workspaceKeys.all(workspaceId) });
    } finally {
      setIsReloading(false);
    }
  }

  // Live updates: the workspace event stream reports job progress and every
  // change other people make, so the page reloads only what an event touched.
  const liveHub = useLiveEventHub(`/v1/workspaces/${encodeURIComponent(workspace.workspace.id)}/events`, accessToken);
  const isLive = useLiveEvents(onLiveEvent, liveHub);
  useEffect(() => () => {
    if (liveRefresh.current.timer !== null) window.clearTimeout(liveRefresh.current.timer);
    liveRefresh.current = { ...NO_LIVE_REFRESH };
  }, [workspace.workspace.id]);

  function queueLiveRefresh(parts: Partial<Omit<LiveRefresh, "timer">>) {
    const pending = liveRefresh.current;
    pending.full ||= Boolean(parts.full);
    pending.ledger ||= Boolean(parts.ledger);
    pending.memory ||= Boolean(parts.memory);
    pending.tasks ||= Boolean(parts.tasks);
    pending.metrics ||= Boolean(parts.metrics);
    if (pending.timer !== null) return;
    // Metrics are costly to compute, so progress-only updates wait longer.
    const delay = pending.full || pending.ledger || pending.memory || pending.tasks ? LIVE_REFRESH_DELAY_MS : LIVE_METRICS_DELAY_MS;
    pending.timer = window.setTimeout(() => void flushLiveRefresh(), delay);
  }

  async function flushLiveRefresh() {
    const pending = liveRefresh.current;
    liveRefresh.current = { ...NO_LIVE_REFRESH };
    if (pending.full) { await load(); return; }
    const stale: Array<readonly unknown[]> = [
      ...(pending.ledger ? workspaceInvalidation.ledger(workspaceId) : []),
      ...(pending.memory ? workspaceInvalidation.memory(workspaceId) : []),
      ...(pending.tasks ? workspaceInvalidation.tasks(workspaceId) : []),
    ];
    // Lists update at once; the figures follow a little later, once per burst.
    if (pending.ledger || pending.memory || pending.tasks) queueLiveRefresh({ metrics: true });
    if (pending.metrics) stale.push(...workspaceInvalidation.metrics(workspaceId));
    try {
      await invalidateKeys(stale);
    } catch { /* the next event, a reconnect or the fallback poll catches up */ }
  }

  function onLiveEvent(event: LiveEvent) {
    if (event.type === "resync") { queueLiveRefresh({ full: true }); return; }
    if (event.type === "job") {
      const { job } = event;
      const running = job.status === "running" || job.status === "pending";
      setLiveJobs((current) => {
        const next = { ...current };
        if (running) next[job.id] = job; else delete next[job.id];
        return next;
      });
      const kind = job.kind || "indexing";
      if (!running && kind === "indexing") queueLiveRefresh({ ledger: true });
      // Embedding progress moves the "passages embedded" figures.
      else if (kind === "embedding" && (!running || isSettingsView)) queueLiveRefresh({ metrics: true });
      return;
    }
    const { event: activityEvent } = event;
    if (LEDGER_ACTIONS.has(activityEvent.action)) queueLiveRefresh({ ledger: true });
    if (MEMORY_ACTIONS.has(activityEvent.action)) queueLiveRefresh({ memory: true });
    if (TASK_ACTIONS.has(activityEvent.action)) queueLiveRefresh({ tasks: true });
    if (isActivityView && canManageMembers) {
      queryClient.setQueryData<WorkspaceActivityEvent[]>(workspaceKeys.activity(workspaceId), (current) => [activityEvent, ...(current ?? []).filter((entry) => entry.id !== activityEvent.id)].slice(0, 100));
    }
  }

  async function stopJob(job: IndexingJobStatus) {
    setError(null);
    try {
      await cancelSharedJob(accessToken, job.id);
      setLiveJobs((current) => current[job.id] ? { ...current, [job.id]: { ...current[job.id], cancel_requested: true } } : current);
      showToast("success", "Stopping. The job ends after its current step.");
    } catch (requestError) { setError(apiMessage(requestError)); }
  }

  // New evidence is indexed by the server in the background. While anything is
  // still waiting, refresh the ledger until it is done, then give up quietly so
  // an artifact that failed to index does not keep the page polling forever.
  // With the live stream open, events do the refreshing and this only runs
  // now and then as a safety net.
  useEffect(() => {
    setIndexingStalled(false);
    if (!pendingIndexKey) return;
    const interval = isLive ? LIVE_FALLBACK_POLL_MS : INDEX_POLL_INTERVAL_MS;
    const maxAttempts = Math.ceil((INDEX_POLL_ATTEMPTS * INDEX_POLL_INTERVAL_MS) / interval);
    let attempts = 0;
    const timer = window.setInterval(async () => {
      attempts += 1;
      try {
        const [nextArtifacts, nextFailures] = await Promise.all([
          listSharedArtifacts(accessToken, workspace.workspace.id),
          listSharedIndexFailures(accessToken, workspace.workspace.id),
        ]);
        setArtifacts(nextArtifacts);
        const failed = Object.fromEntries(nextFailures.map((failure) => [failure.artifact_id, failure]));
        setIndexFailures(failed);
        if (nextArtifacts.every((artifact) => artifact.indexed_at || failed[artifact.id])) {
          window.clearInterval(timer);
          const [nextOverview, nextMetrics, nextFacets] = await Promise.all([
            getSharedWorkspaceOverview(accessToken, workspace.workspace.id),
            getSharedWorkspaceMetrics(accessToken, workspace.workspace.id),
            getSharedRetrievalFacets(accessToken, workspace.workspace.id),
          ]);
          setOverview(nextOverview); setWorkspaceMetrics(nextMetrics); setRetrievalFacets(nextFacets);
          return;
        }
      } catch { /* keep the last known state and try again */ }
      if (attempts >= maxAttempts) {
        window.clearInterval(timer);
        setIndexingStalled(true);
      }
    }, interval);
    return () => window.clearInterval(timer);
  }, [accessToken, isLive, pendingIndexKey, workspace.workspace.id]);

  /** Opens the "save as memory" dialog with an AI answer and its sources. */
  function draftMemoryFromAnswer(title: string, body: string, source: string, citations: Citation[]) {
    setError(null);
    memoryDraftOrigin.current = { title: title.slice(0, 200), body };
    setMemoryDraft({ title: title.slice(0, 200), body, source, citations, selected: new Set(citations.map((_, index) => index)) });
  }

  async function saveMemoryDraft() {
    if (!memoryDraft) return;
    setIsSubmitting(true); setError(null);
    try {
      const card = await createSharedMemoryCard(accessToken, workspace.workspace.id, {
        title: memoryDraft.title.trim(),
        bodyMarkdown: memoryDraft.body,
        source: memoryDraft.source,
        citations: memoryDraft.citations.filter((_, index) => memoryDraft.selected.has(index)),
      });
      setMemoryDraft(null);
      setMemoryCards(await listSharedMemoryCards(accessToken, workspace.workspace.id));
      showToast("success", `Saved to team memory: ${card.title}.`);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  const pendingLedgerCount = useMemo(() => artifacts.filter((artifact) => !artifact.repository_id && !artifact.indexed_at && !indexFailures[artifact.id]).length, [artifacts, indexFailures]);
  // Per-file indexing shows on each file; longer jobs get a progress row.
  const backgroundJobs = useMemo(() => Object.values(liveJobs).filter((job) => job.kind && job.kind !== "indexing").sort((a, b) => a.created_at.localeCompare(b.created_at)), [liveJobs]);
  const liveStatus = pendingLedgerCount || backgroundJobs.length ? <section aria-label="Background work" aria-live="polite" className="shared-live-status">
    {pendingLedgerCount ? <div className="shared-live-row">{indexingStalled ? <AlertCircle size={ICON_SIZE.sm} /> : <span aria-hidden="true" className="rm-indexing"><i /><i /><i /></span>}<span>{indexingStalled ? `${pendingLedgerCount} ${pendingLedgerCount === 1 ? "file is" : "files are"} not indexed yet. The server retries them later.` : `Indexing ${pendingLedgerCount} ${pendingLedgerCount === 1 ? "file" : "files"}…`}</span>{isLive ? null : <small>Live updates unavailable · checking every few seconds</small>}</div> : null}
    {backgroundJobs.map((job) => {
      const progress = job.progress_total ? Math.min(100, Math.round((job.progress_current / job.progress_total) * 100)) : undefined;
      return <div className="rm-repo-progress shared-live-job" key={job.id} role="status">
        <div><span>{JOB_KIND_LABEL[job.kind ?? ""] ?? job.kind} · {JOB_STAGE_LABEL[job.stage] ?? job.stage.replace(/_/g, " ")}</span><span>{job.progress_total ? `${job.progress_current.toLocaleString()} / ${job.progress_total.toLocaleString()}` : ""}</span></div>
        <div className="shared-live-job-bar"><progress aria-label={JOB_KIND_LABEL[job.kind ?? ""] ?? "Job progress"} max={100} value={progress} />{canWrite ? <Button disabled={job.cancel_requested} onClick={() => void stopJob(job)} type="button" variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop"}</Button> : null}</div>
      </div>;
    })}
  </section> : null;

  const memoryDraftDialog = memoryDraft ? <Dialog size="lg" description="Keep this answer as a durable team memory card. Its cited passages become the card's evidence links." footer={<><DialogCancel onClick={closeMemoryDraft} /><Button busy={isSubmitting} disabled={!memoryDraft.title.trim() || !memoryDraft.body.trim()} type="submit" variant="main"><Shield size={ICON_SIZE.md} /> Save memory</Button></>} onClose={closeMemoryDraft} onSubmit={() => { if (memoryDraft.title.trim() && memoryDraft.body.trim()) void saveMemoryDraft(); }} open title="Save as memory">
    <Field label="Title" required><Input maxLength={200} onChange={(event) => setMemoryDraft((current) => current ? { ...current, title: event.target.value } : current)} value={memoryDraft.title} /></Field>
    <Field hint={submitHint("saves")} label="Statement" required><Textarea className="shared-memory-draft-body" onChange={(event) => setMemoryDraft((current) => current ? { ...current, body: event.target.value } : current)} value={memoryDraft.body} /></Field>
    {memoryDraft.citations.length ? <fieldset className="shared-memory-draft-citations"><legend>Evidence links ({memoryDraft.selected.size} of {memoryDraft.citations.length})</legend>{memoryDraft.citations.map((citation, index) => <Checkbox checked={memoryDraft.selected.has(index)} key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}-${index}`} label={<span>[{index + 1}] {citation.title} · {citation.path}{citationLines(citation)}</span>} onCheckedChange={() => setMemoryDraft((current) => { if (!current) return current; const selected = new Set(current.selected); if (selected.has(index)) selected.delete(index); else selected.add(index); return { ...current, selected }; })} />)}</fieldset> : null}
  </Dialog> : null;

  const page: WorkspacePage = {
    accessToken, workspace, workspaceId, session, section,
    canWrite, canManageMembers, canAssignAdmin, canManageWorkspace, canInspectIndex, isWorkspaceAdmin,
    isLive, isLoading, isSubmitting, indexingStalled, liveStatus,
    setIsSubmitting, setError, load, onOpenArtifact, onOpenMemoryCard, onNavigate, onWorkspaceUpdated, onWorkspaceDeleted,
    draftMemoryFromAnswer, store,
  };

  return (
    <LiveEventsContext.Provider value={liveHub}>
    <SharedLayout aside={<AssistantDock accessToken={accessToken} key={workspace.workspace.id} onOpenArtifact={onOpenArtifact} workspaceId={workspace.workspace.id} />} apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizationId={organization?.id} organizations={organizations} workspaceId={workspace.workspace.id} workspaces={workspaces} />} workspaceNavigation={<WorkspaceTopbar activeSection={section} onNavigate={onNavigate} workspace={workspace} />}>
      <section className="shared-detail-shell" aria-busy={isLoading}>
        <PageHeader description={isEvidenceView ? "Store notes and files in the workspace. New evidence is indexed automatically, so it is ready for retrieval shortly after it is saved." : isDocumentsView ? "Upload Word, Excel, PowerPoint, PDF, OneNote and Outlook files, preview them here, and keep them alongside the workspace evidence ledger." : isRetrievalView ? "Search indexed workspace evidence and inspect the source context behind every result." : isMapView ? "See how far your files are through indexing and search by meaning, and how their content connects." : isRepositoriesView ? "Git repositories linked to this workspace. Each sync indexes what the branch has committed and keeps every file in step: edits update it in place, renames keep its comments and memory links, and deleted files leave search." : isReviewView ? "Everything that waits for a person in one list: files marked for review, memory cards whose evidence changed and open health findings. Each opens where it is resolved and leaves the list when it is." : isHealthView ? "Deterministic checks for stale versions, duplicates, documents that drifted from the code, and files search cannot see. Administrators decide what happens to each finding." : isMemoryView ? "Capture concise facts and decisions that should outlive the current investigation." : isTasksView ? "Turn evidence and decisions into assigned, trackable action items for the team." : isPeopleView ? "See who can access this workspace, and review what people and the AI have done." : isActivityView ? "A durable record of shared workspace changes, including evidence, memory, indexing, and membership updates." : isSettingsView ? "Administrators control workspace access and AI integrations here. Owner-only actions remain clearly marked." : "Artifacts, search results, and durable team memory are all retrieved through the protected shared API."} title={isEvidenceView ? "Evidence ledger" : isDocumentsView ? "Documents" : isRetrievalView ? "Retrieve evidence" : isMapView ? "Knowledge map" : isHealthView ? "Workspace health" : isReviewView ? "Review queue" : isRepositoriesView ? "Repositories" : isMemoryView ? "Durable team memory" : isTasksView ? "Tasks" : isPeopleView ? "People & Activity" : isSettingsView ? "Workspace settings" : workspace.workspace.name} />
        <Toast kind="error" message={error} />
        {memoryDraftDialog}
        {isRestrictedView ? <div className="shared-empty-state shared-admin-restricted-state"><Shield size={25} /><strong>Workspace administrator access required</strong><span>People, Activity, and Settings are reserved for workspace owners and administrators.</span><Button onClick={() => onNavigate("overview")} type="button" variant="secondary">Go to overview</Button></div> : <>
        {section === "overview" ? <OverviewSection key={workspaceId} page={page} /> : null}
        {isMapView ? <KnowledgeMapPanel accessToken={accessToken} canConfigure={isWorkspaceAdmin} key={workspace.workspace.id} onOpenArtifact={onOpenArtifact} onOpenEvidence={() => onNavigate("evidence")} onOpenMemoryCard={onOpenMemoryCard} onOpenSettings={() => onNavigate("settings")} workspaceId={workspace.workspace.id} /> : null}
        {isReviewView ? <ReviewQueuePanel accessToken={accessToken} canWrite={canWrite} isWorkspaceAdmin={isWorkspaceAdmin} key={workspace.workspace.id} onOpenArtifact={onOpenArtifact} onOpenHealth={() => onNavigate("health")} onOpenMemoryCard={onOpenMemoryCard} workspaceId={workspace.workspace.id} /> : null}
        {isHealthView ? <WorkspaceHealthPanel accessToken={accessToken} canAct={isWorkspaceAdmin} key={workspace.workspace.id} onOpenArtifact={onOpenArtifact} workspaceId={workspace.workspace.id} /> : null}
        {isRepositoriesView ? <RepositoriesPanel accessToken={accessToken} canManage={isWorkspaceAdmin} canSync={canWrite} key={workspace.workspace.id} onChanged={() => void load()} onOpenArtifact={onOpenArtifact} onOpenSettings={() => onNavigate("settings")} workspaceId={workspace.workspace.id} /> : null}
        {section !== "overview" && !isMapView && !isHealthView && !isReviewView && !isRepositoriesView ? <div className={`shared-detail-grid${isEvidenceView || isDocumentsView ? " evidence-only" : isRetrievalView ? " retrieval-only" : isMemoryView ? " memory-only" : isTasksView ? " tasks-only" : isPeopleView ? " people-only" : isSettingsView ? " settings-only" : ""}`}>
          {isEvidenceView || isDocumentsView ? <EvidenceSection key={workspaceId} page={page} /> : null}
          {isRetrievalView || isMemoryView || isTasksView || isPeopleView ? <aside className="shared-detail-panel shared-retrieval-panel">
            {isRetrievalView ? <RetrievalSection key={workspaceId} page={page} /> : null}
            {isMemoryView ? <MemorySection key={workspaceId} page={page} /> : null}
            {isTasksView ? <TasksSection key={workspaceId} page={page} /> : null}
            {isPeopleView ? <PeopleSection key={workspaceId} page={page} /> : null}
          </aside> : null}
          {isSettingsView ? <SettingsSection key={workspaceId} page={page} /> : null}
        </div> : null}
        </>}
      </section>
    </SharedLayout>
    </LiveEventsContext.Provider>
  );
}