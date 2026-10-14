import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { SafeMarkdown } from "../components/SafeMarkdown";
import { IconAlertCircle as AlertCircle, IconBook2 as Book, IconBrain as Brain, IconChartBar as Chart, IconChecklist as Checklist, IconChevronDown as ChevronDown, IconChevronRight as ChevronRight, IconCode as Code, IconFileText as FileText, IconFilter as Filter, IconFolder as FolderIcon, IconFolderPlus as FolderPlus, IconFolderSymlink as FolderMove, IconLayoutGrid as Grid, IconList as List, IconLoader2 as Loader, IconMarkdown as Markdown, IconNote as Note, IconPencil as Pencil, IconPlus as Plus, IconRefresh as Refresh, IconSearch as Search, IconSettings as Settings, IconShieldLock as Shield, IconSparkles as Sparkles, IconTimeline as Timeline, IconTrash as Trash, IconUpload as Upload, IconUsers as Users } from "@tabler/icons-react";
import { SharedApiError, askSharedWorkspace, cancelSharedJob, createSharedCollaborationTask, createSharedFolder, createSharedMemoryCard, createSharedSavedSearch, createSharedTaskChecklistItem, createSharedTextArtifact, deleteSharedArtifact, deleteSharedCollaborationTask, deleteSharedFolder, deleteSharedSavedSearch, deleteSharedWorkspace, generateSharedWorkspaceAiOverview, getSharedRetrievalFacets, getSharedWorkspaceActivityCalendar, getSharedWorkspaceCapabilities, getSharedWorkspaceMetrics, getSharedWorkspaceOverview, listSharedArtifacts, listSharedCollaborationTasks, listSharedFolders, listSharedIndexFailures, listSharedMemoryCards, listSharedSavedSearches, listSharedTaskChecklist, listSharedWorkspaceActivity, listSharedWorkspaceAiProviders, listSharedWorkspaceJobs, listSharedWorkspaceMembers, moveSharedArtifact, removeSharedWorkspaceMember, renameSharedFolder, searchSharedMemoryCards, searchSharedWorkspace, toggleSharedTaskChecklistItem, updateSharedCollaborationTask, updateSharedWorkspace, uploadSharedArtifact, upsertSharedWorkspaceMember } from "../lib/sharedApi";
import { type ArtifactIndexFailure, type ArtifactSummary, type ArtifactType, type AskAnswer, type Citation, type CollaborationTask, type CollaborationTaskPriority, type CollaborationTaskStatus, type Folder, type IndexingJobStatus, type LiveEvent, type MemoryCardSummary, type Organization, type RetrievalFacets, type SavedSearch, type SearchResult, type SharedAiProviderSettings, type SharedSession, type SharedWorkspace, type TaskChecklistItem, type WorkspaceActivityCalendar, type WorkspaceActivityEvent, type WorkspaceAiOverview, type WorkspaceCapabilities, type WorkspaceMember, type WorkspaceMetrics, type WorkspaceOverview, type WorkspaceRole } from "../types";
import { UserAvatar } from "../components/UserAvatar";
import { Button } from "../components/ui/button";
import { Pagination, paginate } from "../components/ui/pagination";
import { ActionMenu } from "../components/ui/action-menu";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { Dropdown } from "../components/ui/dropdown";
import { Input } from "../components/ui/input";
import { Textarea } from "../components/ui/textarea";
import { DOCUMENT_ACCEPT, isDocumentArtifact } from "../lib/documents";
import { AiPolicySettings } from "../components/AiPolicySettings";
import { AiProviderForm } from "../components/AiProviderForm";
import { AssistantDock } from "../components/AssistantDock";
import { DocumentViewer, KnowledgeMapPanel, RepositoriesPanel, RepositorySettings, RichNoteEditor, WorkspaceHealthPanel } from "./lazy";
import { Toast, showToast } from "../components/ui/toast";
import { LiveEventsContext, useLiveEventHub, useLiveEvents } from "../lib/liveEvents";
import { InviteMembers } from "../components/InviteMembers";
import { SharedLayout } from "../components/SharedLayout";
import { ADMIN_WORKSPACE_SECTIONS, type ArtifactViewMode, FILE_CATEGORIES, type FileCategory, INDEX_POLL_ATTEMPTS, INDEX_POLL_INTERVAL_MS, type ItemDialog, JOB_KIND_LABEL, JOB_STAGE_LABEL, LEDGER_ACTIONS, LIVE_FALLBACK_POLL_MS, LIVE_METRICS_DELAY_MS, LIVE_REFRESH_DELAY_MS, type LiveRefresh, MAX_FOLDER_DEPTH, MAX_UPLOAD_BYTES, MEMORY_ACTIONS, type MemoryDraft, NO_LIVE_REFRESH, SKIPPED_UPLOAD_FOLDERS, TASK_ACTIONS, TASK_COLUMNS, UPLOAD_CONCURRENCY, UPLOAD_EXTENSIONS, type UploadProgress, type WorkspaceSection } from "./constants";
import { apiMessage, artifactCitation, artifactKindLabel, artifactTypeLabel, citationLines, documentKindLabel, fileCategory, formatActivityTime, formatFileSize, formatTaskDueDate, isAiActivity, isTaskOverdue } from "./format";
import { navigate } from "./router";
import { selectUploads } from "../lib/uploadFilter";
import { invalidateKeys, queryClient, workspaceInvalidation, workspaceKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import { ContributionCalendar, EvidenceUsed, MetricBars, MetricTimeline, fileCategoryIcon, renderHighlightedSnippet } from "./common";
import { OrganizationRail, WorkspaceTopbar } from "./Organization";

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
  const [artifactQuery, setArtifactQuery] = useState("");
  const [artifactTypes, setArtifactTypes] = useState<FileCategory[]>([]);
  const [artifactPage, setArtifactPage] = useState(0);
  const [resultsPage, setResultsPage] = useState(0);
  const [artifactViewMode, setArtifactViewMode] = useState<ArtifactViewMode>("grid");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [query, setQuery] = useState("");
  const [retrievalArtifactType, setRetrievalArtifactType] = useState<ArtifactType | "all">("all");
  const [retrievalLanguage, setRetrievalLanguage] = useState("all");
  const [retrievalSourceId, setRetrievalSourceId] = useState("all");
  const [retrievalLimit, setRetrievalLimit] = useState("20");
  const [savedSearchName, setSavedSearchName] = useState("");
  const [askQuestion, setAskQuestion] = useState("");
  const [askAnswer, setAskAnswer] = useState<AskAnswer | null>(null);
  const [askedQuestion, setAskedQuestion] = useState("");
  const [memoryQuery, setMemoryQuery] = useState("");
  const [memoryResults, setMemoryResults] = useState<MemoryCardSummary[] | null>(null);
  const [noteTitle, setNoteTitle] = useState("");
  const [noteContent, setNoteContent] = useState("");
  const [uploadFiles, setUploadFiles] = useState<File[]>([]);
  const [uploadProgress, setUploadProgress] = useState<UploadProgress | null>(null);
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null);
  const [memoryTitle, setMemoryTitle] = useState("");
  const [memoryBody, setMemoryBody] = useState("");
  const [memoryArtifactIds, setMemoryArtifactIds] = useState<string[]>([]);
  const [memoryDraft, setMemoryDraft] = useState<MemoryDraft | null>(null);
  const [liveJobs, setLiveJobs] = useState<Record<string, IndexingJobStatus>>({});
  const liveRefresh = useRef<LiveRefresh>({ ...NO_LIVE_REFRESH });
  const [activityKind, setActivityKind] = useState<"user" | "ai">("user");
  const [taskChecklists, setTaskChecklists] = useState<Record<string, TaskChecklistItem[]>>({});
  const [expandedTaskId, setExpandedTaskId] = useState<string | null>(null);
  const [checklistBody, setChecklistBody] = useState("");
  const [taskQuery, setTaskQuery] = useState("");
  const [taskAssigneeFilter, setTaskAssigneeFilter] = useState("all");
  const [taskTitle, setTaskTitle] = useState("");
  const [taskDescription, setTaskDescription] = useState("");
  const [taskPriority, setTaskPriority] = useState<CollaborationTaskPriority>("medium");
  const [taskAssigneeId, setTaskAssigneeId] = useState("");
  const [taskArtifactId, setTaskArtifactId] = useState("");
  const [taskDueDate, setTaskDueDate] = useState("");
  const [aiOverview, setAiOverview] = useState<WorkspaceAiOverview | null>(null);
  const [currentFolderId, setCurrentFolderId] = useState<string | null>(null);
  const [newFolderOpen, setNewFolderOpen] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");
  const [itemDialog, setItemDialog] = useState<ItemDialog | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [moveTarget, setMoveTarget] = useState<string | null>(null);
  const [memberEmail, setMemberEmail] = useState("");
  const [memberRole, setMemberRole] = useState<WorkspaceRole>("member");
  const [inviteEmail, setInviteEmail] = useState("");
  const [workspaceName, setWorkspaceName] = useState(workspace.workspace.name);
  const [error, setError] = useState<string | null>(null);
  const [isReloading, setIsReloading] = useState(false);
  // Server state lives in the query cache (see lib/queryClient): it is shared
  // with other pages, refreshed in the background, and marked stale by live
  // events and mutations instead of the page reloading everything.
  const workspaceId = workspace.workspace.id;
  const [capabilities, setCapabilities, capabilitiesQuery] = useServerState<WorkspaceCapabilities | null>({
    key: workspaceKeys.capabilities(workspaceId), fetch: () => getSharedWorkspaceCapabilities(accessToken, workspaceId), initial: null,
  });
  const [overview, setOverview, overviewQuery] = useServerState<WorkspaceOverview | null>({
    key: workspaceKeys.overview(workspaceId), fetch: () => getSharedWorkspaceOverview(accessToken, workspaceId), initial: null,
  });
  const [workspaceMetrics, setWorkspaceMetrics, metricsQuery] = useServerState<WorkspaceMetrics | null>({
    key: workspaceKeys.metrics(workspaceId), fetch: () => getSharedWorkspaceMetrics(accessToken, workspaceId), initial: null,
  });
  const [artifacts, setArtifacts, artifactsQuery] = useServerState<ArtifactSummary[]>({
    key: workspaceKeys.artifacts(workspaceId), fetch: () => listSharedArtifacts(accessToken, workspaceId), initial: [],
  });
  const [indexFailures, setIndexFailures] = useServerState<Record<string, ArtifactIndexFailure>>({
    key: workspaceKeys.indexFailures(workspaceId),
    fetch: async () => Object.fromEntries((await listSharedIndexFailures(accessToken, workspaceId)).map((failure) => [failure.artifact_id, failure])),
    initial: {},
  });
  const [folders, setFolders, foldersQuery] = useServerState<Folder[]>({
    key: workspaceKeys.folders(workspaceId), fetch: () => listSharedFolders(accessToken, workspaceId), initial: [],
  });
  const [memoryCards, setMemoryCards, memoryCardsQuery] = useServerState<MemoryCardSummary[]>({
    key: workspaceKeys.memoryCards(workspaceId), fetch: () => listSharedMemoryCards(accessToken, workspaceId), initial: [],
  });
  const [retrievalFacets, setRetrievalFacets, facetsQuery] = useServerState<RetrievalFacets | null>({
    key: workspaceKeys.facets(workspaceId), fetch: () => getSharedRetrievalFacets(accessToken, workspaceId), initial: null,
  });
  const [, , runningJobsQuery] = useServerState<IndexingJobStatus[]>({
    // Running jobs seed the live progress; the stream keeps them current.
    key: workspaceKeys.runningJobs(workspaceId),
    fetch: () => listSharedWorkspaceJobs(accessToken, workspaceId, "running").catch(() => [] as IndexingJobStatus[]),
    initial: [],
  });
  const canSeeMembers = capabilities?.can_manage_members ?? false;
  const [members, setMembers] = useServerState<WorkspaceMember[]>({
    key: workspaceKeys.members(workspaceId), fetch: () => listSharedWorkspaceMembers(accessToken, workspaceId), initial: [], enabled: canSeeMembers,
  });
  const [aiProviders, setAiProviders] = useServerState<SharedAiProviderSettings[]>({
    key: workspaceKeys.aiProviders(workspaceId), fetch: () => listSharedWorkspaceAiProviders(accessToken, workspaceId), initial: [], enabled: canSeeMembers,
  });
  const [activity, setActivity] = useServerState<WorkspaceActivityEvent[]>({
    key: workspaceKeys.activity(workspaceId), fetch: () => listSharedWorkspaceActivity(accessToken, workspaceId), initial: [], enabled: section === "people" && canSeeMembers,
  });
  const [activityCalendar, setActivityCalendar] = useServerState<WorkspaceActivityCalendar | null>({
    key: workspaceKeys.activityCalendar(workspaceId), fetch: () => getSharedWorkspaceActivityCalendar(accessToken, workspaceId), initial: null, enabled: section === "people" && canSeeMembers,
  });
  const [tasks, setTasks] = useServerState<CollaborationTask[]>({
    key: workspaceKeys.tasks(workspaceId), fetch: () => listSharedCollaborationTasks(accessToken, workspaceId), initial: [], enabled: section === "tasks",
  });
  const [savedSearches, setSavedSearches] = useServerState<SavedSearch[]>({
    key: workspaceKeys.savedSearches(workspaceId), fetch: () => listSharedSavedSearches(accessToken, workspaceId), initial: [], enabled: section === "retrieval",
  });
  const coreQueries = [capabilitiesQuery, overviewQuery, metricsQuery, artifactsQuery, foldersQuery, memoryCardsQuery, facetsQuery];
  // Only the first load, or a reload someone asked for, counts as loading; background refreshes stay quiet.
  const isLoading = isReloading || coreQueries.some((query) => query.isLoading);
  const loadError = coreQueries.find((query) => query.error)?.error ?? null;
  useEffect(() => { if (loadError) setError(apiMessage(loadError)); }, [loadError]);
  // Running jobs seed the live progress panel once per fetch.
  useEffect(() => {
    if (runningJobsQuery.data) setLiveJobs(Object.fromEntries(runningJobsQuery.data.map((job) => [job.id, job])));
  }, [runningJobsQuery.dataUpdatedAt]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const canWrite = capabilities?.can_write_content ?? false;
  const canManageMembers = capabilities?.can_manage_members ?? false;
  const canAssignAdmin = capabilities?.can_assign_admin ?? false;
  const canManageWorkspace = capabilities?.can_manage_workspace ?? false;
  const canInspectIndex = capabilities?.can_inspect_index ?? false;
  const [indexingStalled, setIndexingStalled] = useState(false);
  const pendingIndexKey = useMemo(() => artifacts.filter((artifact) => !artifact.indexed_at && !indexFailures[artifact.id]).map((artifact) => artifact.id).join(","), [artifacts, indexFailures]);
  const isEvidenceView = section === "evidence";
  const isDocumentsView = section === "documents";
  const isRepositoriesView = section === "repositories";
  const isRetrievalView = section === "retrieval";
  const isMapView = section === "map";
  const isHealthView = section === "health";
  const isMemoryView = section === "memory";
  const isTasksView = section === "tasks";
  const aiActivity = useMemo(() => activity.filter(isAiActivity), [activity]);
  const userActivity = useMemo(() => activity.filter((event) => !isAiActivity(event)), [activity]);
  const visibleActivity = activityKind === "ai" ? aiActivity : userActivity;
  const isPeopleView = section === "people";
  const isActivityView = isPeopleView;
  const isSettingsView = section === "settings";
  const isWorkspaceAdmin = workspace.role === "owner" || workspace.role === "admin";
  const isRestrictedView = ADMIN_WORKSPACE_SECTIONS.has(section) && !isWorkspaceAdmin;
  const displayedArtifacts = useMemo(() => {
    const normalizedQuery = artifactQuery.trim().toLowerCase();
    return artifacts.filter((artifact) => {
      const matchesQuery = !normalizedQuery || `${artifact.title} ${artifact.path}`.toLowerCase().includes(normalizedQuery);
      const matchesType = artifactTypes.length === 0 || artifactTypes.includes(fileCategory(artifact));
      const inFolder = !isEvidenceView || Boolean(normalizedQuery) || artifactTypes.length > 0 || (artifact.folder_id ?? null) === currentFolderId;
      return matchesQuery && matchesType && inFolder && !artifact.repository_id && (!isDocumentsView || isDocumentArtifact(artifact));
    });
  }, [artifactQuery, artifactTypes, artifacts, currentFolderId, isDocumentsView, isEvidenceView]);
  const pagedArtifacts = useMemo(() => paginate(displayedArtifacts, artifactPage), [artifactPage, displayedArtifacts]);
  // A new search, filter, folder or section starts again from the first page.
  useEffect(() => { setResultsPage(0); }, [results]);
  useEffect(() => { setArtifactPage(0); }, [artifactQuery, artifactTypes, currentFolderId, section, workspace.workspace.id]);
  const hasFolderFilters = Boolean(artifactQuery.trim() || artifactTypes.length);
  const folderById = useMemo(() => new Map(folders.map((folder) => [folder.id, folder])), [folders]);
  const folderPath = useMemo(() => {
    const path: Folder[] = [];
    let cursor = currentFolderId ? folderById.get(currentFolderId) : undefined;
    while (cursor) {
      path.unshift(cursor);
      cursor = cursor.parent_id ? folderById.get(cursor.parent_id) : undefined;
    }
    return path;
  }, [currentFolderId, folderById]);
  const visibleFolders = useMemo(
    () => isEvidenceView && !hasFolderFilters ? folders.filter((folder) => (folder.parent_id ?? null) === currentFolderId) : [],
    [currentFolderId, folders, hasFolderFilters, isEvidenceView],
  );
  const folderItemCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const folder of folders) if (folder.parent_id) counts.set(folder.parent_id, (counts.get(folder.parent_id) ?? 0) + 1);
    for (const artifact of artifacts) if (artifact.folder_id) counts.set(artifact.folder_id, (counts.get(artifact.folder_id) ?? 0) + 1);
    return counts;
  }, [artifacts, folders]);
  const activeFolderId = isEvidenceView ? currentFolderId : null;
  // Repository files are browsed in the Repositories section, not the ledger.
  const documentArtifacts = useMemo(() => artifacts.filter((artifact) => !artifact.repository_id && isDocumentArtifact(artifact)), [artifacts]);
  const ledgerArtifactCount = useMemo(() => artifacts.filter((artifact) => !artifact.repository_id).length, [artifacts]);
  const selectedDocument = documentArtifacts.find((artifact) => artifact.id === selectedDocumentId) ?? null;
  const hasArtifactFilters = Boolean(artifactQuery.trim() || artifactTypes.length);
  const visibleTasks = tasks.filter((task) => {
    const normalizedQuery = taskQuery.trim().toLowerCase();
    const matchesQuery = !normalizedQuery || `${task.title} ${task.description}`.toLowerCase().includes(normalizedQuery);
    const matchesAssignee = taskAssigneeFilter === "all"
      || (taskAssigneeFilter === "unassigned" ? !task.assignee : task.assignee?.id === taskAssigneeFilter);
    return matchesQuery && matchesAssignee;
  });

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
      setActivity((current) => [activityEvent, ...current.filter((entry) => entry.id !== activityEvent.id)].slice(0, 100));
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
  const embeddedChunks = workspaceMetrics?.embedded_chunk_count ?? null;
  const totalChunks = workspaceMetrics?.chunk_count ?? 0;
  const embeddingPending = embeddedChunks !== null && embeddedChunks < totalChunks;
  const embeddingStatus = embeddedChunks === null
    ? null
    : totalChunks === 0
      ? "No indexed passages yet. They are embedded as soon as files are indexed."
      : embeddingPending
        ? `${embeddedChunks.toLocaleString()} of ${totalChunks.toLocaleString()} passages embedded so far…`
        : `All ${totalChunks.toLocaleString()} indexed passages are searchable by meaning.`;
  // Embedding runs in the background on the server; keep the settings
  // progress current while it does, and stop after a while if it stalls.
  // Embedding job events refresh it while the live stream is open.
  useEffect(() => {
    if (!isSettingsView || !embeddingPending || isLive) return;
    let attempts = 0;
    const timer = window.setInterval(async () => {
      attempts += 1;
      try { setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id)); } catch { /* try again */ }
      if (attempts >= INDEX_POLL_ATTEMPTS) window.clearInterval(timer);
    }, INDEX_POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [accessToken, embeddingPending, isLive, isSettingsView, workspace.workspace.id]);
  useEffect(() => { setCurrentFolderId(null); setNewFolderOpen(false); }, [workspace.workspace.id]);
  useEffect(() => { setWorkspaceName(workspace.workspace.name); }, [workspace.workspace.name]);
  useEffect(() => {
    if (!isDocumentsView) {
      return;
    }

    setSelectedDocumentId((current) =>
      current && documentArtifacts.some((artifact) => artifact.id === current)
        ? current
        : documentArtifacts[0]?.id ?? null,
    );
  }, [documentArtifacts, isDocumentsView]);

  async function addNote(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!noteContent.trim()) { setError("Write something in the note body before storing it."); return; }
    setIsSubmitting(true); setError(null);
    try {
      await createSharedTextArtifact(accessToken, workspace.workspace.id, {
        title: noteTitle,
        content: noteContent,
        language: "Note",
        folder_id: activeFolderId,
      });
      setNoteTitle(""); setNoteContent("");
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function addMemory(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      await createSharedMemoryCard(accessToken, workspace.workspace.id, {
        title: memoryTitle,
        bodyMarkdown: memoryBody,
        source: "Team note",
        citations: memoryArtifactIds.flatMap((artifactId) => artifactCitation(artifacts, artifactId)),
      });
      setMemoryTitle(""); setMemoryBody(""); setMemoryArtifactIds([]);
      await load();
      showToast("success", "Memory saved.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  /** Opens the "save as memory" dialog with an AI answer and its sources. */
  function draftMemoryFromAnswer(title: string, body: string, source: string, citations: Citation[]) {
    setError(null);
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

  async function addTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      await createSharedCollaborationTask(accessToken, workspace.workspace.id, {
        title: taskTitle,
        description: taskDescription,
        status: "open",
        priority: taskPriority,
        assigneeUserId: taskAssigneeId || undefined,
        artifactId: taskArtifactId || undefined,
        dueAt: taskDueDate ? `${taskDueDate}T23:59:59Z` : undefined,
      });
      setTaskTitle(""); setTaskDescription(""); setTaskPriority("medium"); setTaskAssigneeId(""); setTaskArtifactId(""); setTaskDueDate("");
      setTasks(await listSharedCollaborationTasks(accessToken, workspace.workspace.id));
      setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function reviseTask(task: CollaborationTask, changes: { status?: CollaborationTaskStatus; priority?: CollaborationTaskPriority; assigneeUserId?: string | null }) {
    setIsSubmitting(true); setError(null);
    try {
      const updated = await updateSharedCollaborationTask(accessToken, task.id, {
        title: task.title,
        description: task.description,
        status: changes.status ?? task.status,
        priority: changes.priority ?? task.priority,
        assigneeUserId: changes.assigneeUserId === null ? undefined : changes.assigneeUserId ?? task.assignee?.id,
        artifactId: task.artifact_id ?? undefined,
        dueAt: task.due_at ?? undefined,
      });
      setTasks((current) => current.map((entry) => entry.id === updated.id ? updated : entry));
      setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeTask(task: CollaborationTask) {
    if (!window.confirm(`Delete “${task.title}”?`)) return;
    setIsSubmitting(true); setError(null);
    try {
      await deleteSharedCollaborationTask(accessToken, task.id);
      setTasks((current) => current.filter((entry) => entry.id !== task.id));
      setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  /** Keeps the files the upload accepts and says why the others are left out.
   *  Files from a chosen folder keep their folder path (`webkitRelativePath`). */
  function chooseUploadFiles(list: FileList | null) {
    const { kept, unsupported, tooLarge, ignored } = selectUploads(Array.from(list ?? []), isDocumentsView);
    const notes = [
      unsupported ? `${unsupported} of an unsupported type` : "",
      tooLarge ? `${tooLarge} over 10 MiB` : "",
      ignored ? `${ignored} in hidden, dependency or build folders` : "",
    ].filter(Boolean);
    if (notes.length) showToast("warning", `${notes.join(", ")} left out.`);
    setUploadFiles(kept);
  }

  /** Finds or creates the folder chain for a file chosen inside a folder,
   *  below the folder being viewed. Deeper levels than allowed go into the
   *  deepest folder that fits. */
  async function uploadFolderFor(file: File, created: Map<string, string>, known: Folder[]): Promise<string | null> {
    const segments = (file.webkitRelativePath || "").split("/").slice(0, -1);
    let parentId = activeFolderId;
    const room = MAX_FOLDER_DEPTH - (activeFolderId ? folderPath.length : 0);
    for (let index = 0; index < Math.min(segments.length, Math.max(0, room)); index += 1) {
      const key = segments.slice(0, index + 1).join("/");
      const cached = created.get(key);
      if (cached) { parentId = cached; continue; }
      const name = segments[index].slice(0, 80);
      const existing = known.find((folder) => (folder.parent_id ?? null) === parentId && folder.name === name);
      const folder = existing ?? await createSharedFolder(accessToken, workspace.workspace.id, { name, parentId });
      if (!existing) known.push(folder);
      created.set(key, folder.id);
      parentId = folder.id;
    }
    return parentId;
  }

  async function submitUpload(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const files = uploadFiles;
    if (!files.length) return;
    setIsSubmitting(true); setError(null);
    setUploadProgress({ done: 0, total: files.length, failed: 0 });
    const failures: string[] = [];
    let lastDocument: ArtifactSummary | null = null;
    let uploaded = 0;
    try {
      // Folders are created one at a time first, so parallel uploads never
      // race to create the same one.
      const created = new Map<string, string>();
      const known = [...folders];
      const targets: Array<{ file: File; folderId: string | null }> = [];
      for (const file of files) targets.push({ file, folderId: await uploadFolderFor(file, created, known) });
      if (created.size) setFolders(known);
      let next = 0;
      const worker = async () => {
        while (next < targets.length) {
          const { file, folderId } = targets[next++];
          try {
            const artifact = await uploadSharedArtifact(accessToken, workspace.workspace.id, file, folderId);
            uploaded += 1;
            if (isDocumentArtifact(artifact)) lastDocument = artifact;
          } catch (requestError) {
            failures.push(`${file.name}: ${apiMessage(requestError)}`);
          }
          setUploadProgress({ done: uploaded + failures.length, total: targets.length, failed: failures.length });
        }
      };
      await Promise.all(Array.from({ length: Math.min(UPLOAD_CONCURRENCY, targets.length) }, worker));
      const uploadedDocument = lastDocument as ArtifactSummary | null;
      if (isDocumentsView && uploadedDocument) setSelectedDocumentId(uploadedDocument.id);
      setUploadFiles([]);
      form.reset();
      await load();
      if (uploaded) showToast("success", uploaded === 1 ? "File uploaded. It is indexed in the background." : `${uploaded} files uploaded. They are indexed in the background.`);
      if (failures.length) setError(failures.length === 1 ? failures[0] : `${failures.length} files were not uploaded. ${failures.slice(0, 3).join(" · ")}${failures.length > 3 ? " …" : ""}`);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); setUploadProgress(null); }
  }

  async function runSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!query.trim()) return;
    setIsSubmitting(true); setError(null);
    try {
      setResults(await searchSharedWorkspace(accessToken, workspace.workspace.id, {
        query,
        artifactTypes: retrievalArtifactType === "all" ? [] : [retrievalArtifactType],
        languages: retrievalLanguage === "all" ? [] : [retrievalLanguage],
        sourceIds: retrievalSourceId === "all" ? [] : [retrievalSourceId],
        limit: Number(retrievalLimit),
      }));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function clearRetrievalFilters() {
    setRetrievalArtifactType("all");
    setRetrievalLanguage("all");
    setRetrievalSourceId("all");
    setRetrievalLimit("20");
    setResults([]);
  }

  async function saveCurrentSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      const saved = await createSharedSavedSearch(accessToken, workspace.workspace.id, { name: savedSearchName, query, artifactTypes: retrievalArtifactType === "all" ? [] : [retrievalArtifactType], languages: retrievalLanguage === "all" ? [] : [retrievalLanguage], sourceIds: retrievalSourceId === "all" ? [] : [retrievalSourceId], resultLimit: Number(retrievalLimit) });
      setSavedSearches((current) => [saved, ...current]); setSavedSearchName("");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function applySavedSearch(saved: SavedSearch) {
    setQuery(saved.query); setRetrievalArtifactType(saved.artifact_types[0] ?? "all"); setRetrievalLanguage(saved.languages[0] ?? "all"); setRetrievalSourceId(saved.source_ids[0] ?? "all"); setRetrievalLimit(String(saved.result_limit)); setResults([]);
  }

  async function removeSavedSearch(saved: SavedSearch) {
    setIsSubmitting(true); setError(null);
    try { await deleteSharedSavedSearch(accessToken, saved.id); setSavedSearches((current) => current.filter((entry) => entry.id !== saved.id)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function toggleTaskChecklist(task: CollaborationTask) {
    if (expandedTaskId === task.id) { setExpandedTaskId(null); return; }
    setIsSubmitting(true); setError(null);
    try { const checklist = await listSharedTaskChecklist(accessToken, task.id); setTaskChecklists((current) => ({ ...current, [task.id]: checklist })); setExpandedTaskId(task.id); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function addChecklistItem(task: CollaborationTask, event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setIsSubmitting(true); setError(null);
    try { const item = await createSharedTaskChecklistItem(accessToken, task.id, checklistBody); setTaskChecklists((current) => ({ ...current, [task.id]: [...(current[task.id] ?? []), item] })); setChecklistBody(""); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function toggleChecklistItem(item: TaskChecklistItem) {
    setIsSubmitting(true); setError(null);
    try { const updated = await toggleSharedTaskChecklistItem(accessToken, item.id, !item.completed_at); setTaskChecklists((current) => ({ ...current, [item.task_id]: (current[item.task_id] ?? []).map((entry) => entry.id === updated.id ? updated : entry) })); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function toggleArtifactType(type: FileCategory) {
    setArtifactTypes((current) => current.includes(type)
      ? current.filter((currentType) => currentType !== type)
      : [...current, type]);
  }

  async function askEvidence(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!askQuestion.trim()) return;
    setIsSubmitting(true); setError(null);
    const question = askQuestion.trim();
    try { setAskAnswer(await askSharedWorkspace(accessToken, workspace.workspace.id, askQuestion)); setAskedQuestion(question); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function runMemorySearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextQuery = memoryQuery.trim();
    if (!nextQuery) { setMemoryResults(null); return; }
    setIsSubmitting(true); setError(null);
    try { setMemoryResults(await searchSharedMemoryCards(accessToken, workspace.workspace.id, nextQuery)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function generateAiOverview() {
    setIsSubmitting(true); setError(null);
    try { setAiOverview(await generateSharedWorkspaceAiOverview(accessToken, workspace.workspace.id)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function renameWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      const updated = await updateSharedWorkspace(accessToken, workspace.workspace.id, workspaceName);
      onWorkspaceUpdated(updated);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeWorkspace() {
    if (!window.confirm(`Delete ${workspace.workspace.name}? This permanently removes all shared evidence and memory in this workspace.`)) return;
    setIsSubmitting(true); setError(null);
    try {
      await deleteSharedWorkspace(accessToken, workspace.workspace.id);
      onWorkspaceDeleted(workspace.workspace.id);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function addFolder(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      const folder = await createSharedFolder(accessToken, workspace.workspace.id, { name: newFolderName, parentId: currentFolderId });
      setFolders((current) => [...current, folder]);
      setNewFolderName(""); setNewFolderOpen(false);
      showToast("success", `Folder "${folder.name}" created.`);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function openItemDialog(next: ItemDialog) {
    setError(null);
    if (next.kind === "rename-folder") setRenameValue(next.folder.name);
    if (next.kind === "move-file") setMoveTarget(next.artifact.folder_id ?? null);
    setItemDialog(next);
  }

  /** The folder and everything below it, with the number of files inside. */
  function folderSubtree(rootId: string) {
    const ids = new Set([rootId]);
    let grew = true;
    while (grew) {
      grew = false;
      for (const folder of folders) {
        if (folder.parent_id && ids.has(folder.parent_id) && !ids.has(folder.id)) { ids.add(folder.id); grew = true; }
      }
    }
    const files = artifacts.filter((artifact) => artifact.folder_id && ids.has(artifact.folder_id)).length;
    return { ids, files, subfolders: ids.size - 1 };
  }

  async function runItemAction(action: () => Promise<void>) {
    setIsSubmitting(true); setError(null);
    try { await action(); setItemDialog(null); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  const renameFolderAction = (folder: Folder) => runItemAction(async () => {
    const renamed = await renameSharedFolder(accessToken, workspace.workspace.id, folder.id, renameValue);
    setFolders((current) => current.map((entry) => entry.id === renamed.id ? renamed : entry));
    showToast("success", "Folder renamed.");
  });

  const deleteFolderAction = (folder: Folder) => runItemAction(async () => {
    const { ids } = folderSubtree(folder.id);
    await deleteSharedFolder(accessToken, workspace.workspace.id, folder.id);
    if (currentFolderId && ids.has(currentFolderId)) setCurrentFolderId(folder.parent_id);
    await load();
    showToast("success", `Folder "${folder.name}" deleted.`);
  });

  const deleteFileAction = (artifact: ArtifactSummary) => runItemAction(async () => {
    await deleteSharedArtifact(accessToken, artifact.id);
    if (selectedDocumentId === artifact.id) setSelectedDocumentId(null);
    await load();
    showToast("success", `"${artifact.title}" deleted.`);
  });

  const moveFileAction = (artifact: ArtifactSummary) => runItemAction(async () => {
    await moveSharedArtifact(accessToken, artifact.id, moveTarget);
    setArtifacts((current) => current.map((entry) => entry.id === artifact.id ? { ...entry, folder_id: moveTarget } : entry));
    showToast("success", moveTarget ? `Moved to ${folderById.get(moveTarget)?.name ?? "folder"}.` : "Moved to the top level.");
  });

  const folderPickerRows = (() => {
    const rows: Array<{ folder: Folder; depth: number }> = [];
    const walk = (parentId: string | null, depth: number) => {
      for (const folder of folders.filter((entry) => (entry.parent_id ?? null) === parentId)) {
        rows.push({ folder, depth });
        walk(folder.id, depth + 1);
      }
    };
    walk(null, 0);
    return rows;
  })();

  const itemDialogs = (() => {
    if (!itemDialog) return null;
    const busy = isSubmitting;
    if (itemDialog.kind === "rename-folder") {
      const { folder } = itemDialog;
      return <Dialog footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button disabled={busy || !renameValue.trim()} onClick={() => void renameFolderAction(folder)} type="button" variant="main">Rename</Button></>} onClose={() => setItemDialog(null)} open title="Rename folder">
        <Input aria-label="Folder name" autoFocus maxLength={80} onChange={(event) => setRenameValue(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && renameValue.trim()) void renameFolderAction(folder); }} value={renameValue} />
      </Dialog>;
    }
    if (itemDialog.kind === "delete-folder") {
      const { folder } = itemDialog;
      const { files, subfolders } = folderSubtree(folder.id);
      const contents = files || subfolders
        ? `This also permanently deletes ${files} ${files === 1 ? "file" : "files"} and ${subfolders} ${subfolders === 1 ? "subfolder" : "subfolders"} inside it.`
        : "The folder is empty.";
      return <Dialog description={<>Delete the folder <strong>{folder.name}</strong>? {contents} This cannot be undone.</>} footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button className="rm-button-danger" disabled={busy} onClick={() => void deleteFolderAction(folder)} type="button" variant="secondary">Delete folder</Button></>} onClose={() => setItemDialog(null)} open title="Delete folder" />;
    }
    if (itemDialog.kind === "delete-file") {
      const { artifact } = itemDialog;
      return <Dialog description={<>Permanently delete <strong>{artifact.title}</strong> and its index? Links to it from memory cards are removed too. This cannot be undone.</>} footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button className="rm-button-danger" disabled={busy} onClick={() => void deleteFileAction(artifact)} type="button" variant="secondary">Delete file</Button></>} onClose={() => setItemDialog(null)} open title="Delete file" />;
    }
    const { artifact } = itemDialog;
    const unchanged = moveTarget === (artifact.folder_id ?? null);
    return <Dialog description={<>Choose where to put <strong>{artifact.title}</strong>.</>} footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button disabled={busy || unchanged} onClick={() => void moveFileAction(artifact)} type="button" variant="main">Move here</Button></>} onClose={() => setItemDialog(null)} open title="Move to folder">
      <div className="rm-folder-picker" role="group" aria-label="Destination folder">
        <button aria-pressed={moveTarget === null} onClick={() => setMoveTarget(null)} type="button"><FolderIcon size={16} /> All evidence (top level)</button>
        {folderPickerRows.map(({ folder, depth }) => <button aria-pressed={moveTarget === folder.id} key={folder.id} onClick={() => setMoveTarget(folder.id)} style={{ paddingLeft: 10 + (depth + 1) * 16 }} type="button"><FolderIcon size={16} /> {folder.name}</button>)}
      </div>
    </Dialog>;
  })();

  const pendingLedgerCount = useMemo(() => artifacts.filter((artifact) => !artifact.repository_id && !artifact.indexed_at && !indexFailures[artifact.id]).length, [artifacts, indexFailures]);
  // Per-file indexing shows on each file; longer jobs get a progress row.
  const backgroundJobs = useMemo(() => Object.values(liveJobs).filter((job) => job.kind && job.kind !== "indexing").sort((a, b) => a.created_at.localeCompare(b.created_at)), [liveJobs]);
  const liveStatus = pendingLedgerCount || backgroundJobs.length ? <section aria-label="Background work" aria-live="polite" className="shared-live-status">
    {pendingLedgerCount ? <div className="shared-live-row">{indexingStalled ? <AlertCircle size={15} /> : <span aria-hidden="true" className="rm-indexing"><i /><i /><i /></span>}<span>{indexingStalled ? `${pendingLedgerCount} ${pendingLedgerCount === 1 ? "file is" : "files are"} not indexed yet. The server retries them later.` : `Indexing ${pendingLedgerCount} ${pendingLedgerCount === 1 ? "file" : "files"}…`}</span>{isLive ? null : <small>Live updates unavailable · checking every few seconds</small>}</div> : null}
    {backgroundJobs.map((job) => {
      const progress = job.progress_total ? Math.min(100, Math.round((job.progress_current / job.progress_total) * 100)) : undefined;
      return <div className="rm-repo-progress shared-live-job" key={job.id} role="status">
        <div><span>{JOB_KIND_LABEL[job.kind ?? ""] ?? job.kind} · {JOB_STAGE_LABEL[job.stage] ?? job.stage.replace(/_/g, " ")}</span><span>{job.progress_total ? `${job.progress_current.toLocaleString()} / ${job.progress_total.toLocaleString()}` : ""}</span></div>
        <div className="shared-live-job-bar"><progress aria-label={JOB_KIND_LABEL[job.kind ?? ""] ?? "Job progress"} max={100} value={progress} />{canWrite ? <Button disabled={job.cancel_requested} onClick={() => void stopJob(job)} type="button" variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop"}</Button> : null}</div>
      </div>;
    })}
  </section> : null;

  const memoryDraftDialog = memoryDraft ? <Dialog className="rm-dialog-wide" description="Keep this answer as a durable team memory card. Its cited passages become the card's evidence links." footer={<><DialogCancel onClick={() => setMemoryDraft(null)} /><Button disabled={isSubmitting || !memoryDraft.title.trim() || !memoryDraft.body.trim()} onClick={() => void saveMemoryDraft()} type="button" variant="main">{isSubmitting ? <Loader className="spin" size={16} /> : <Shield size={16} />} Save memory</Button></>} onClose={() => setMemoryDraft(null)} open title="Save as memory">
    <label className="shared-dialog-field">Title<Input maxLength={200} onChange={(event) => setMemoryDraft((current) => current ? { ...current, title: event.target.value } : current)} required value={memoryDraft.title} /></label>
    <label className="shared-dialog-field">Statement<Textarea className="shared-memory-draft-body" onChange={(event) => setMemoryDraft((current) => current ? { ...current, body: event.target.value } : current)} required value={memoryDraft.body} /></label>
    {memoryDraft.citations.length ? <fieldset className="shared-memory-draft-citations"><legend>Evidence links ({memoryDraft.selected.size} of {memoryDraft.citations.length})</legend>{memoryDraft.citations.map((citation, index) => <label key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}-${index}`}><input checked={memoryDraft.selected.has(index)} onChange={() => setMemoryDraft((current) => { if (!current) return current; const selected = new Set(current.selected); if (selected.has(index)) selected.delete(index); else selected.add(index); return { ...current, selected }; })} type="checkbox" /><span>[{index + 1}] {citation.title} · {citation.path}{citationLines(citation)}</span></label>)}</fieldset> : null}
  </Dialog> : null;

  function onProviderSaved(saved: SharedAiProviderSettings) {
    setAiProviders((current) => [saved, ...current.filter((entry) => entry.id !== saved.id)]);
    // Saving an image provider makes the server retry images that were waiting.
    void load();
  }

  async function saveMember(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      await upsertSharedWorkspaceMember(accessToken, workspace.workspace.id, {
        email: memberEmail.trim(),
        role: memberRole,
      });
      setMemberEmail("");
      setMemberRole("member");
      await load();
    } catch (requestError) {
      // Not in the organization yet (with or without an account): they join by invitation.
      if (requestError instanceof SharedApiError && requestError.status === 409) {
        setInviteEmail(memberEmail.trim());
        showToast("warning", `${memberEmail.trim()} is not in this organization yet. Create an invite link for them below.`);
      } else setError(apiMessage(requestError));
    } finally { setIsSubmitting(false); }
  }

  async function removeMember(userId: string) {
    setIsSubmitting(true); setError(null);
    try { await removeSharedWorkspaceMember(accessToken, workspace.workspace.id, userId); await load(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
    <LiveEventsContext.Provider value={liveHub}>
    <SharedLayout aside={<AssistantDock accessToken={accessToken} key={workspace.workspace.id} onOpenArtifact={onOpenArtifact} workspaceId={workspace.workspace.id} />} apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizationId={organization?.id} organizations={organizations} workspaceId={workspace.workspace.id} workspaces={workspaces} />} workspaceNavigation={<WorkspaceTopbar activeSection={section} onNavigate={onNavigate} workspace={workspace} />}>
      <section className="shared-detail-shell" aria-busy={isLoading}>
        <div className="shared-detail-heading">
          <div><h1>{isEvidenceView ? "Evidence ledger" : isDocumentsView ? "Documents" : isRetrievalView ? "Retrieve evidence" : isMapView ? "Knowledge map" : isHealthView ? "Workspace health" : isRepositoriesView ? "Repositories" : isMemoryView ? "Durable team memory" : isTasksView ? "Tasks" : isPeopleView ? "People & Activity" : isSettingsView ? "Workspace settings" : workspace.workspace.name}</h1><p>{isEvidenceView ? "Store notes and files in the workspace. New evidence is indexed automatically, so it is ready for retrieval shortly after it is saved." : isDocumentsView ? "Upload Word, Excel, PowerPoint, PDF, OneNote and Outlook files, preview them here, and keep them alongside the workspace evidence ledger." : isRetrievalView ? "Search indexed workspace evidence and inspect the source context behind every result." : isMapView ? "See how far your files are through indexing and search by meaning, and how their content connects." : isRepositoriesView ? "Git repositories linked to this workspace. Each sync indexes what the branch has committed and keeps every file in step: edits update it in place, renames keep its comments and memory links, and deleted files leave search." : isHealthView ? "Deterministic checks for stale versions, duplicates, documents that drifted from the code, and files search cannot see. Administrators decide what happens to each finding." : isMemoryView ? "Capture concise facts and decisions that should outlive the current investigation." : isTasksView ? "Turn evidence and decisions into assigned, trackable action items for the team." : isPeopleView ? "See who can access this workspace, and review what people and the AI have done." : isActivityView ? "A durable record of shared workspace changes, including evidence, memory, indexing, and membership updates." : isSettingsView ? "Administrators control workspace access and AI integrations here. Owner-only actions remain clearly marked." : "Artifacts, search results, and durable team memory are all retrieved through the protected shared API."}</p></div>
        </div>
        <Toast kind="error" message={error} />
        {memoryDraftDialog}
        {isRestrictedView ? <div className="shared-empty-state shared-admin-restricted-state"><Shield size={25} /><strong>Workspace administrator access required</strong><span>People, Activity, and Settings are reserved for workspace owners and administrators.</span><Button onClick={() => onNavigate("overview")} type="button" variant="secondary">Go to overview</Button></div> : <>
        {section === "overview" ? <><div className="shared-evidence-summary">
          <span><strong>{overview?.artifact_count ?? "—"}</strong> artifacts</span>{canInspectIndex ? <span><strong>{overview?.chunk_count ?? "—"}</strong> indexed chunks</span> : null}<span><strong>{overview?.memory_card_count ?? "—"}</strong> memory cards</span>
        </div>
        <section className="shared-metrics-dashboard" aria-labelledby="workspace-metrics-heading">
          <div className="shared-panel-heading"><div><Chart size={18} /><h2 id="workspace-metrics-heading">Workspace pulse</h2></div><span>{workspaceMetrics ? "Live API snapshot" : "Loading metrics…"}</span></div>
          <div className="shared-metric-highlights">
            <div><span>Index coverage</span><strong>{workspaceMetrics ? `${workspaceMetrics.artifact_count ? Math.round((workspaceMetrics.indexed_artifact_count / workspaceMetrics.artifact_count) * 100) : 0}%` : "—"}</strong><small>{workspaceMetrics ? `${workspaceMetrics.indexed_artifact_count} of ${workspaceMetrics.artifact_count} artifacts` : "Waiting for server data"}</small></div>
            <div><span>Indexed storage</span><strong>{workspaceMetrics ? formatFileSize(workspaceMetrics.indexed_artifact_bytes) : "—"}</strong><small>{workspaceMetrics ? `${formatFileSize(workspaceMetrics.pending_artifact_bytes)} pending` : "Waiting for server data"}</small></div>
            <div><span>Fresh evidence</span><strong>{workspaceMetrics?.artifacts_updated_last_7_days ?? "—"}</strong><small>{workspaceMetrics ? `${workspaceMetrics.artifacts_created_last_7_days} added in 7 days` : "Waiting for server data"}</small></div>
            <div><span>Knowledge density</span><strong>{workspaceMetrics ? `${workspaceMetrics.indexed_artifact_count ? Math.round(workspaceMetrics.chunk_count / workspaceMetrics.indexed_artifact_count) : 0}` : "—"}</strong><small>retrieval chunks per indexed file</small></div>
            <div><span>Code symbols</span><strong>{workspaceMetrics?.symbol_count ?? "—"}</strong><small>extracted from evidence</small></div>
            <div><span>Team access</span><strong>{workspaceMetrics?.member_count ?? "—"}</strong><small>people with workspace access</small></div>
            <div><span>Active tasks</span><strong>{workspaceMetrics ? workspaceMetrics.open_task_count + workspaceMetrics.in_progress_task_count : "—"}</strong><small>{workspaceMetrics?.in_progress_task_count ?? 0} currently in progress</small></div>
            <div><span>At risk</span><strong>{workspaceMetrics?.blocked_task_count ?? "—"}</strong><small>{workspaceMetrics?.overdue_task_count ?? 0} overdue action items</small></div>
            <div><span>Team discussion</span><strong>{workspaceMetrics?.comment_count ?? "—"}</strong><small>{workspaceMetrics?.completed_task_count ?? 0} tasks completed</small></div>
          </div>
          <div className="shared-metric-visuals">
            <section className="shared-metric-panel shared-metric-coverage"><div><h3>Indexing coverage</h3><span>{workspaceMetrics ? `${workspaceMetrics.pending_artifact_count} waiting to be indexed` : "Loading…"}</span></div><div className="shared-coverage-track" aria-label="Artifact indexing coverage"><span style={{ width: `${workspaceMetrics?.artifact_count ? (workspaceMetrics.indexed_artifact_count / workspaceMetrics.artifact_count) * 100 : 0}%` }} /></div><div className="shared-coverage-legend"><span>Indexed {workspaceMetrics?.indexed_artifact_count ?? 0}</span><span>Pending {workspaceMetrics?.pending_artifact_count ?? 0}</span></div></section>
            <MetricBars emptyLabel="Evidence types appear after you add files." items={workspaceMetrics?.artifact_types ?? []} title="Evidence by type" />
            <MetricBars emptyLabel="Storage distribution appears after you add files." formatValue={formatFileSize} items={workspaceMetrics?.artifact_bytes_by_type ?? []} title="Storage by evidence type" />
            <MetricBars emptyLabel="Languages appear after indexed files are detected." items={workspaceMetrics?.languages ?? []} title="Languages in evidence" />
            <MetricTimeline items={workspaceMetrics?.activity_by_day ?? []} />
            <MetricBars emptyLabel="Workspace changes will be summarized here." items={workspaceMetrics?.activity_actions ?? []} title="Recent activity" />
            <MetricBars emptyLabel="Team roles appear when members join this workspace." items={workspaceMetrics?.member_roles ?? []} summary={workspaceMetrics ? `${workspaceMetrics.member_count} people` : undefined} title="Team access" />
          </div>
        </section></> : null}
        {section === "overview" ? <section className="shared-ai-overview" aria-live="polite">
          <div className="shared-panel-heading"><div><Brain size={18} /><h2>AI workspace overview</h2></div>{aiOverview?.provider_name ? <span>{aiOverview.provider_name}</span> : null}</div>
          {aiOverview?.summary_markdown ? <div className="shared-markdown shared-ai-answer"><SafeMarkdown>{aiOverview.summary_markdown}</SafeMarkdown></div> : <p className="shared-muted-copy">Generate a concise project briefing from indexed evidence. RepoMemo only uses an enabled provider configured for this workspace and returns the source citations with the result.</p>}
          {aiOverview?.warnings.length ? <div className="shared-ai-overview-warning">{aiOverview.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}
          {aiOverview?.citations.length ? <EvidenceUsed citations={aiOverview.citations} onOpenArtifact={onOpenArtifact} /> : null}
          <div className="shared-ai-actions"><Button disabled={isSubmitting || isLoading || !capabilities?.can_generate_ai_overview} onClick={() => void generateAiOverview()} type="button" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Brain size={16} />}{aiOverview?.summary_markdown ? "Refresh AI overview" : "Generate AI overview"}</Button>{canWrite && aiOverview?.summary_markdown ? <Button disabled={isSubmitting} onClick={() => draftMemoryFromAnswer("Workspace overview", aiOverview.summary_markdown ?? "", "AI workspace overview", aiOverview.citations)} type="button" variant="secondary"><Shield size={16} /> Save as memory</Button> : null}</div>
        </section> : null}
        {isMapView ? <KnowledgeMapPanel accessToken={accessToken} canConfigure={isWorkspaceAdmin} key={workspace.workspace.id} onOpenArtifact={onOpenArtifact} onOpenEvidence={() => onNavigate("evidence")} onOpenMemoryCard={onOpenMemoryCard} onOpenSettings={() => onNavigate("settings")} workspaceId={workspace.workspace.id} /> : null}
        {isHealthView ? <WorkspaceHealthPanel accessToken={accessToken} canAct={isWorkspaceAdmin} key={workspace.workspace.id} onOpenArtifact={onOpenArtifact} workspaceId={workspace.workspace.id} /> : null}
        {isRepositoriesView ? <RepositoriesPanel accessToken={accessToken} canManage={isWorkspaceAdmin} canSync={canWrite} key={workspace.workspace.id} onChanged={() => void load()} onOpenArtifact={onOpenArtifact} onOpenSettings={() => onNavigate("settings")} workspaceId={workspace.workspace.id} /> : null}
        {section !== "overview" && !isMapView && !isHealthView && !isRepositoriesView ? <div className={`shared-detail-grid${isEvidenceView || isDocumentsView ? " evidence-only" : isRetrievalView ? " retrieval-only" : isMemoryView ? " memory-only" : isTasksView ? " tasks-only" : isPeopleView ? " people-only" : isSettingsView ? " settings-only" : ""}`}>
          {isEvidenceView || isDocumentsView ? <section className="shared-detail-panel">
            <div className="shared-artifact-browser">
              {itemDialogs}
              <div className="shared-artifact-filters">
                <Input aria-label="Search evidence files" onChange={(event) => setArtifactQuery(event.target.value)} placeholder="Search file name or path" value={artifactQuery} />
                <details className="shared-artifact-type-menu">
                  <summary><Filter size={16} /><span>{artifactTypes.length ? `${artifactTypes.length} types` : "File types"}</span><ChevronDown size={16} /></summary>
                  <div className="shared-artifact-type-menu-panel">
                    <label><input checked={artifactTypes.length === 0} onChange={() => setArtifactTypes([])} type="checkbox" /><span>All types</span></label>
                    {FILE_CATEGORIES.map((option) => {
                      const isSelected = artifactTypes.includes(option.value);
                      return <label key={option.value}><input checked={isSelected} onChange={() => toggleArtifactType(option.value)} type="checkbox" />{fileCategoryIcon(option.value, 15)}<span>{option.label}</span></label>;
                    })}
                  </div>
                </details>
                {isEvidenceView && canWrite ? <Button disabled={folderPath.length >= MAX_FOLDER_DEPTH} onClick={() => setNewFolderOpen((open) => !open)} title={folderPath.length >= MAX_FOLDER_DEPTH ? `Folders can be nested up to ${MAX_FOLDER_DEPTH} levels deep` : "Create a folder here"} type="button" variant="secondary"><FolderPlus size={16} /> New folder</Button> : null}
              </div>
              {isEvidenceView && newFolderOpen ? <form className="shared-folder-form" onSubmit={addFolder}><Input aria-label="Folder name" autoFocus maxLength={80} onChange={(event) => setNewFolderName(event.target.value)} placeholder={folderPath.length ? `New folder in ${folderPath[folderPath.length - 1].name}` : "New folder name"} required value={newFolderName} /><Button disabled={isSubmitting} type="submit" variant="main">Create</Button><Button onClick={() => { setNewFolderOpen(false); setNewFolderName(""); }} type="button" variant="secondary">Cancel</Button></form> : null}
              {liveStatus}<div className="shared-artifact-browser-meta"><div className="shared-artifact-browser-summary">{isEvidenceView && !hasFolderFilters && (folders.length || currentFolderId) ? <nav aria-label="Folder path" className="shared-folder-path"><button className={currentFolderId ? "" : "current"} onClick={() => setCurrentFolderId(null)} type="button">All evidence</button>{folderPath.map((folder, index) => <span key={folder.id}><ChevronRight size={14} /><button className={index === folderPath.length - 1 ? "current" : ""} onClick={() => setCurrentFolderId(folder.id)} type="button">{folder.name}</button></span>)}</nav> : null}<span>{isEvidenceView && !hasFolderFilters ? `${visibleFolders.length} ${visibleFolders.length === 1 ? "folder" : "folders"} · ${displayedArtifacts.length} ${displayedArtifacts.length === 1 ? "file" : "files"}` : `${displayedArtifacts.length} of ${isDocumentsView ? documentArtifacts.length : ledgerArtifactCount} files${hasArtifactFilters ? " matching filters" : ""}`}</span></div><div className="shared-artifact-view-switch" role="group" aria-label="Evidence view"><Button aria-label="Grid view" aria-pressed={artifactViewMode === "grid"} className={artifactViewMode === "grid" ? "active" : ""} onClick={() => setArtifactViewMode("grid")} type="button" variant="secondary"><Grid size={16} /></Button><Button aria-label="List view" aria-pressed={artifactViewMode === "list"} className={artifactViewMode === "list" ? "active" : ""} onClick={() => setArtifactViewMode("list")} type="button" variant="secondary"><List size={16} /></Button></div></div>
              {displayedArtifacts.length || visibleFolders.length ? <div className={`shared-artifact-manager ${artifactViewMode}`}>{visibleFolders.map((folder) => <article key={folder.id}><Button className="shared-artifact-entry shared-folder-entry" onClick={() => setCurrentFolderId(folder.id)} type="button" variant="secondary"><span className="shared-artifact-file-icon category-folder"><FolderIcon size={20} /></span><span className="shared-artifact-entry-copy"><strong>{folder.name}</strong></span><span className="shared-artifact-entry-meta"><span>{folderItemCounts.get(folder.id) ?? 0} {(folderItemCounts.get(folder.id) ?? 0) === 1 ? "item" : "items"}</span></span></Button>{canWrite ? <div className="shared-item-actions"><ActionMenu items={[{ label: "Rename", icon: <Pencil size={15} />, onSelect: () => openItemDialog({ kind: "rename-folder", folder }) }, { label: "Delete folder", icon: <Trash size={15} />, destructive: true, onSelect: () => openItemDialog({ kind: "delete-folder", folder }) }]} label={`Actions for folder ${folder.name}`} /></div> : null}</article>)}{pagedArtifacts.map((artifact) => <article key={artifact.id}><Button className={isDocumentsView && artifact.id === selectedDocument?.id ? "shared-artifact-entry selected" : "shared-artifact-entry"} onClick={() => isDocumentsView ? setSelectedDocumentId(artifact.id) : onOpenArtifact(artifact.id)} type="button" variant="secondary"><span className={`shared-artifact-file-icon category-${fileCategory(artifact)}`}>{fileCategoryIcon(fileCategory(artifact), 20)}</span><span className="shared-artifact-entry-copy"><strong>{artifact.title}</strong></span><span className="shared-artifact-entry-meta"><span>{artifactKindLabel(artifact)}</span><span>{formatFileSize(artifact.size_bytes)}</span></span>{artifact.indexed_at ? null : indexFailures[artifact.id] || indexingStalled ? <span aria-label="Not indexed" className="shared-artifact-status failed" role="img" title={indexFailures[artifact.id] ? `Indexing failed: ${indexFailures[artifact.id].message}` : "Indexing problem: this file is not indexed"}><AlertCircle size={16} /></span> : <span aria-label="Indexing" className="shared-artifact-status" role="img" title="Indexing…"><span aria-hidden="true" className="rm-indexing"><i /><i /><i /></span></span>}</Button>{canWrite ? <div className="shared-item-actions"><ActionMenu items={[{ label: "Move to folder…", icon: <FolderMove size={15} />, onSelect: () => openItemDialog({ kind: "move-file", artifact }) }, ...(artifact.artifact_type === "repository" ? [] : [{ label: "Delete file", icon: <Trash size={15} />, destructive: true, onSelect: () => openItemDialog({ kind: "delete-file", artifact }) }])]} label={`Actions for ${artifact.title}`} /></div> : null}</article>)}</div> : <div className="shared-empty-state"><FileText size={25} /><strong>{isEvidenceView && currentFolderId && !hasFolderFilters ? "This folder is empty" : ledgerArtifactCount ? "No files match these filters" : isDocumentsView ? "No documents yet" : "No shared evidence yet"}</strong><span>{isEvidenceView && currentFolderId && !hasFolderFilters ? "Add a note or upload a file below, or create a folder inside it." : ledgerArtifactCount ? "Adjust the search or selected types to see other workspace files." : isDocumentsView ? "Upload a Word, Excel, PowerPoint, PDF, OneNote or Outlook file to preview it here." : "Add a pasted note below, then index it when you are ready to search."}</span></div>}
              <Pagination onPageChange={setArtifactPage} page={artifactPage} total={displayedArtifacts.length} />
            </div>
            {isDocumentsView ? <section className="shared-document-preview" aria-live="polite"><div className="shared-panel-heading"><div><Book size={18} /><h2>Preview</h2></div><span>{selectedDocument ? documentKindLabel(selectedDocument) : `${documentArtifacts.length} documents`}</span></div>{selectedDocument ? <DocumentViewer accessToken={accessToken} artifact={selectedDocument} key={selectedDocument.id} /> : <div className="shared-empty-state"><FileText size={25} /><strong>No document selected</strong><span>Choose a document above to preview it.</span></div>}</section> : null}
            {canWrite ? <div className="shared-evidence-additions">{isEvidenceView ? <form className="shared-note-form" onSubmit={addNote}><h3>Add shared note{activeFolderId ? <small className="shared-folder-hint"> in {folderPath.map((folder) => folder.name).join(" / ")}</small> : null}</h3><Input onChange={(event) => setNoteTitle(event.target.value)} placeholder="Decision or implementation note" required value={noteTitle} /><RichNoteEditor onChange={setNoteContent} placeholder="Write a note. Use the toolbar or Markdown for formatting…" value={noteContent} /><Button disabled={isSubmitting} type="submit" variant="main"><Plus size={16} /> Store evidence</Button></form> : null}<form className="shared-upload-form" onSubmit={submitUpload}><div><strong>{isDocumentsView ? "Upload documents" : "Upload files"}{activeFolderId ? <small className="shared-folder-hint"> to {folderPath.map((folder) => folder.name).join(" / ")}</small> : null}</strong><span>{isDocumentsView ? "Word, Excel, PowerPoint, PDF, OneNote, Outlook · up to 10 MiB each · text is extracted locally for search" : "Markdown, text, code, image, or business document · up to 10 MiB each · choose several files or a whole folder"}</span></div><label className="shared-upload-picker"><input accept={isDocumentsView ? DOCUMENT_ACCEPT : `${UPLOAD_EXTENSIONS.map((extension) => `.${extension}`).join(",")},${DOCUMENT_ACCEPT}`} aria-label={isDocumentsView ? "Upload documents" : "Upload shared files"} className="shared-upload-native-input" disabled={isSubmitting} multiple onChange={(event) => chooseUploadFiles(event.target.files)} type="file" /><span className="shared-upload-picker-icon"><Upload size={18} /></span><span className="shared-upload-picker-copy"><strong>{uploadFiles.length === 1 ? uploadFiles[0].name : uploadFiles.length ? `${uploadFiles.length} files selected` : isDocumentsView ? "Choose documents" : "Choose shared files"}</strong><span>{uploadProgress ? `Uploading ${uploadProgress.done} of ${uploadProgress.total}${uploadProgress.failed ? ` · ${uploadProgress.failed} failed` : ""}…` : uploadFiles.length ? `${formatFileSize(uploadFiles.reduce((total, file) => total + file.size, 0))} · ready to upload` : isDocumentsView ? "Word, Excel, PowerPoint, PDF, OneNote or Outlook" : "Markdown, text, code, image, or business document"}</span></span><span className="shared-upload-picker-action">Browse</span></label>{uploadProgress ? <progress aria-label="Upload progress" className="shared-upload-progress" max={uploadProgress.total} value={uploadProgress.done} /> : null}<div className="shared-upload-actions">{isEvidenceView ? <label className="rm-button rm-button-secondary shared-upload-folder"><FolderIcon size={16} /> Choose a folder<input {...({ webkitdirectory: "", directory: "" } as Record<string, string>)} aria-label="Upload a folder" className="shared-upload-native-input" disabled={isSubmitting} onChange={(event) => chooseUploadFiles(event.target.files)} type="file" /></label> : null}<Button disabled={isSubmitting || !uploadFiles.length} type="submit" variant="secondary">{uploadProgress ? <Loader className="spin" size={16} /> : <Upload size={16} />} {uploadFiles.length > 1 ? `Upload ${uploadFiles.length} files` : "Upload"}</Button></div></form></div> : <p className="shared-readonly-note"><Shield size={15} /> Your viewer membership can inspect shared evidence but cannot change it.</p>}
          </section> : null}
          {isRetrievalView || isMemoryView || isTasksView || isPeopleView || isActivityView ? <aside className="shared-detail-panel shared-retrieval-panel">
            {isRetrievalView ? <><div className="shared-panel-heading"><div><Search size={18} /><h2>Retrieve</h2></div></div>
            <form className="shared-retrieval-search" onSubmit={runSearch}>
              <Input onChange={(event) => setQuery(event.target.value)} placeholder="Search indexed evidence" value={query} />
              <div className="shared-retrieval-filter-row">
                <Dropdown aria-label="Limit retrieval to a file type" onValueChange={(value) => setRetrievalArtifactType(value as ArtifactType | "all")} options={[{ label: "All file types", value: "all" }, ...(retrievalFacets?.artifact_types ?? []).map((type) => ({ label: artifactTypeLabel(type), value: type }))]} value={retrievalArtifactType} />
                <Dropdown aria-label="Limit retrieval to a language" onValueChange={setRetrievalLanguage} options={[{ label: "All languages", value: "all" }, ...(retrievalFacets?.languages ?? []).map((language) => ({ label: language, value: language }))]} value={retrievalLanguage} />
                <Dropdown aria-label="Limit retrieval to a source" onValueChange={setRetrievalSourceId} options={[{ label: "All sources", value: "all" }, ...(retrievalFacets?.sources ?? []).map((source) => ({ label: source.name, value: source.id }))]} value={retrievalSourceId} />
                <Dropdown aria-label="Number of results" onValueChange={setRetrievalLimit} options={[{ label: "10 results", value: "10" }, { label: "20 results", value: "20" }, { label: "50 results", value: "50" }]} value={retrievalLimit} />
                <Button disabled={isSubmitting} type="submit" variant="main"><Search size={16} /> Search</Button>
                {results.length ? <Button disabled={isSubmitting} onClick={clearRetrievalFilters} type="button" variant="secondary">Clear</Button> : null}
              </div>
            </form>
            <section className="shared-saved-searches"><div className="shared-panel-heading"><div><Book size={17} /><h3>Saved searches</h3></div><span>{savedSearches.length}</span></div>{savedSearches.length ? <div className="shared-saved-search-list">{savedSearches.map((saved) => <article key={saved.id}><Button onClick={() => applySavedSearch(saved)} type="button" variant="secondary"><span><strong>{saved.name}</strong><small>{saved.query}</small></span></Button>{canWrite ? <Button aria-label={`Delete ${saved.name}`} disabled={isSubmitting} onClick={() => void removeSavedSearch(saved)} type="button" variant="secondary"><Trash size={14} /></Button> : null}</article>)}</div> : <p className="shared-muted-copy">Save a query and its filters for recurring investigations.</p>}{canWrite ? <form className="shared-save-search-form" onSubmit={saveCurrentSearch}><Input onChange={(event) => setSavedSearchName(event.target.value)} placeholder="Search name" required value={savedSearchName} /><Button disabled={isSubmitting || !query.trim()} type="submit" variant="secondary">Save current search</Button></form> : null}</section>
            <div>{results.length ? <div className="shared-search-results"><p className="shared-search-result-count">{results.length} matching evidence {results.length === 1 ? "result" : "results"}</p>{paginate(results, resultsPage).map((result) => <article key={result.chunk_id}><Button className="shared-search-result" onClick={() => onOpenArtifact(result.artifact_id)} type="button" variant="secondary"><span className="shared-search-result-heading"><strong>{result.title}</strong><span>{artifactTypeLabel(result.artifact_type)} · {result.language ?? "Unspecified"} · {result.source_name}</span></span><p>{renderHighlightedSnippet(result.snippet)}</p><span className="shared-search-result-path">{result.path}{result.start_line ? ` · line ${result.start_line}` : ""}</span></Button></article>)}<Pagination onPageChange={setResultsPage} page={resultsPage} total={results.length} /></div> : <p className="shared-muted-copy">Index one or more artifacts, then search the evidence base from here. Use filters to narrow large workspaces.</p>}
              <section className="shared-ask-evidence" aria-live="polite"><div><Brain size={18} /><h3>Ask your evidence</h3></div><p>Get a citation-backed answer from the indexed workspace context.</p><form onSubmit={askEvidence}><Textarea onChange={(event) => setAskQuestion(event.target.value)} placeholder="What do we know about the current implementation?" required value={askQuestion} /><Button disabled={isSubmitting} type="submit" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Brain size={16} />} Ask</Button></form>{askAnswer ? <div className="shared-ask-answer"><p className="shared-ai-generated-note">AI-generated from the cited passages · verify against the sources</p><div className="shared-markdown shared-ai-answer"><SafeMarkdown>{askAnswer.answer_markdown}</SafeMarkdown></div>{askAnswer.warnings.length ? <div className="shared-ai-overview-warning">{askAnswer.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}{askAnswer.citations.length ? <EvidenceUsed citations={askAnswer.citations} numbered onOpenArtifact={onOpenArtifact} /> : null}{canWrite && askAnswer.citations.length ? <div className="shared-ai-actions"><Button disabled={isSubmitting} onClick={() => draftMemoryFromAnswer(askedQuestion || "Answer from workspace evidence", askAnswer.answer_markdown, "Ask your evidence", askAnswer.citations)} type="button" variant="secondary"><Shield size={16} /> Save as memory</Button></div> : null}</div> : null}</section>
            </div></> : null}
            {isMemoryView ? <div className="shared-memory-section"><div className="shared-panel-heading"><div><Shield size={18} /><h2>Team memory</h2></div><span>{memoryResults?.length ?? memoryCards.length}</span></div><form className="shared-search-form shared-memory-search" onSubmit={runMemorySearch}><Input onChange={(event) => setMemoryQuery(event.target.value)} placeholder="Search team memory" value={memoryQuery} /><Button disabled={isSubmitting} type="submit" variant="secondary">Search</Button></form>{(memoryResults ?? memoryCards).length ? <div className="shared-memory-list">{(memoryResults ?? memoryCards).map((card) => <article key={card.id}><Button className="shared-record-link" onClick={() => onOpenMemoryCard(card.id)} type="button" variant="secondary"><strong>{card.title}</strong><span>{card.body_excerpt}</span></Button></article>)}</div> : <p className="shared-muted-copy">{memoryResults ? "No memory cards match that search." : "No durable memory cards yet."}</p>}{canWrite ? <form className="shared-memory-form" onSubmit={addMemory}><Input onChange={(event) => setMemoryTitle(event.target.value)} placeholder="Memory title" required value={memoryTitle} /><Textarea onChange={(event) => setMemoryBody(event.target.value)} placeholder="A concise durable fact…" required value={memoryBody} /><div className="shared-memory-evidence"><span>Evidence links</span>{memoryArtifactIds.length ? <ul aria-label="Linked evidence">{memoryArtifactIds.map((artifactId) => { const linked = artifacts.find((artifact) => artifact.id === artifactId); return <li key={artifactId}><span>{linked?.title ?? "Missing evidence"}</span><button aria-label={`Remove ${linked?.title ?? "evidence"}`} onClick={() => setMemoryArtifactIds((current) => current.filter((id) => id !== artifactId))} type="button">×</button></li>; })}</ul> : null}<Dropdown aria-label="Add an evidence link" onValueChange={(value) => { if (value !== "__add__") setMemoryArtifactIds((current) => current.includes(value) ? current : [...current, value]); }} options={[{ label: memoryArtifactIds.length ? "Add another evidence link" : "Add an evidence link (optional)", value: "__add__" }, ...artifacts.filter((artifact) => !memoryArtifactIds.includes(artifact.id)).map((artifact) => ({ label: artifact.title, value: artifact.id }))]} value="__add__" /></div><Button disabled={isSubmitting} type="submit" variant="secondary">Save memory</Button></form> : null}</div> : null}
            {isTasksView ? <div className="shared-task-workspace">
              <div className="shared-panel-heading"><div><Checklist size={18} /><h2>Team action board</h2></div><span>{tasks.length} tasks</span></div>
              <div className="shared-task-toolbar"><Input aria-label="Search tasks" onChange={(event) => setTaskQuery(event.target.value)} placeholder="Search action items" value={taskQuery} /><Dropdown aria-label="Filter tasks by assignee" onValueChange={setTaskAssigneeFilter} options={[{ label: "Everyone", value: "all" }, { label: "Unassigned", value: "unassigned" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={taskAssigneeFilter} /></div>
              {canWrite ? <form className="shared-task-create" onSubmit={addTask}><div className="shared-task-create-heading"><div><Plus size={17} /><div><strong>Create an action item</strong><span>Assign work and tie it back to the evidence that prompted it.</span></div></div></div><Input aria-label="Task title" onChange={(event) => setTaskTitle(event.target.value)} placeholder="What needs to happen?" required value={taskTitle} /><Textarea aria-label="Task description" onChange={(event) => setTaskDescription(event.target.value)} placeholder="Context, acceptance criteria, or next steps" value={taskDescription} /><div className="shared-task-create-fields"><Dropdown aria-label="Task priority" onValueChange={(value) => setTaskPriority(value as CollaborationTaskPriority)} options={[{ label: "Low priority", value: "low" }, { label: "Medium priority", value: "medium" }, { label: "High priority", value: "high" }, { label: "Urgent", value: "urgent" }]} value={taskPriority} /><Dropdown aria-label="Task assignee" onValueChange={(value) => setTaskAssigneeId(value === "__unassigned__" ? "" : value)} options={[{ label: "Unassigned", value: "__unassigned__" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={taskAssigneeId || "__unassigned__"} /><Dropdown aria-label="Related evidence" onValueChange={(value) => setTaskArtifactId(value === "__none__" ? "" : value)} options={[{ label: "No evidence link", value: "__none__" }, ...artifacts.map((artifact) => ({ label: artifact.title, value: artifact.id }))]} value={taskArtifactId || "__none__"} /><Input aria-label="Task due date" onChange={(event) => setTaskDueDate(event.target.value)} type="date" value={taskDueDate} /></div><Button disabled={isSubmitting} type="submit" variant="main">Create task</Button></form> : null}
              {visibleTasks.length ? <div className="shared-task-board">{TASK_COLUMNS.map((column) => { const columnTasks = visibleTasks.filter((task) => task.status === column.status); return <section className="shared-task-column" key={column.status}><div className="shared-task-column-heading"><h3>{column.label}</h3><span>{columnTasks.length}</span></div><div className="shared-task-column-list">{columnTasks.length ? columnTasks.map((task) => <article className={`shared-task-card priority-${task.priority}${isTaskOverdue(task) ? " overdue" : ""}`} key={task.id}><div className="shared-task-card-heading"><span>{task.priority}</span>{task.due_at ? <time dateTime={task.due_at}>{formatTaskDueDate(task.due_at)}</time> : null}</div><h4>{task.title}</h4>{task.description ? <p>{task.description}</p> : null}<div className="shared-task-context"><span>{task.assignee?.display_name ?? "Unassigned"}</span>{task.artifact_id ? <Button onClick={() => onOpenArtifact(task.artifact_id!)} type="button" variant="secondary"><FileText size={14} /> Evidence</Button> : null}</div>{canWrite ? <div className="shared-task-controls"><Dropdown aria-label={`Status for ${task.title}`} onValueChange={(value) => void reviseTask(task, { status: value as CollaborationTaskStatus })} options={TASK_COLUMNS.map((option) => ({ label: option.label, value: option.status }))} value={task.status} /><Dropdown aria-label={`Assignee for ${task.title}`} onValueChange={(value) => void reviseTask(task, { assigneeUserId: value === "__unassigned__" ? null : value })} options={[{ label: "Unassigned", value: "__unassigned__" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={task.assignee?.id ?? "__unassigned__"} /><Dropdown aria-label={`Priority for ${task.title}`} onValueChange={(value) => void reviseTask(task, { priority: value as CollaborationTaskPriority })} options={[{ label: "Low", value: "low" }, { label: "Medium", value: "medium" }, { label: "High", value: "high" }, { label: "Urgent", value: "urgent" }]} value={task.priority} />{task.created_by.id === session.user.id || workspace.role === "owner" || workspace.role === "admin" ? <Button aria-label={`Delete ${task.title}`} disabled={isSubmitting} onClick={() => void removeTask(task)} type="button" variant="secondary"><Trash size={15} /></Button> : null}</div> : null}</article>) : <p>No tasks here.</p>}</div></section>; })}</div> : <div className="shared-empty-state"><Checklist size={25} /><strong>No action items match</strong><span>Create a task or adjust the current filters.</span></div>}
              {expandedTaskId ? (() => { const task = tasks.find((entry) => entry.id === expandedTaskId); return task ? <section className="shared-task-checklist-panel"><div className="shared-panel-heading"><div><Checklist size={17} /><h3>{task.title} checklist</h3></div><Button onClick={() => setExpandedTaskId(null)} type="button" variant="secondary">Close</Button></div>{(taskChecklists[task.id] ?? []).length ? <div className="shared-task-checklist-list">{(taskChecklists[task.id] ?? []).map((item) => <label key={item.id}><input checked={Boolean(item.completed_at)} disabled={!canWrite || isSubmitting} onChange={() => void toggleChecklistItem(item)} type="checkbox" /><span>{item.body}</span></label>)}</div> : <p className="shared-muted-copy">No steps yet. Break the work into clear, checkable outcomes.</p>}{canWrite ? <form className="shared-task-checklist-form" onSubmit={(event) => void addChecklistItem(task, event)}><Input onChange={(event) => setChecklistBody(event.target.value)} placeholder="Add a completion step" required value={checklistBody} /><Button disabled={isSubmitting} type="submit" variant="secondary">Add step</Button></form> : null}</section> : null; })() : null}
              <div className="shared-task-checklist-select"><Dropdown aria-label="Open a task checklist" onValueChange={(value) => { const task = tasks.find((entry) => entry.id === value); if (task) void toggleTaskChecklist(task); }} options={[{ label: "Open task checklist", value: "__none__" }, ...tasks.map((task) => ({ label: task.title, value: task.id }))]} value={expandedTaskId ?? "__none__"} /></div>
            </div> : null}
            {isPeopleView ? <div className="shared-members-section">
              <div className="shared-panel-heading"><div><Users size={18} /><h2>Workspace team</h2></div><span>{members.length}</span></div>
              <p className="shared-muted-copy">{canManageMembers ? "Add people and manage their access to this workspace." : "Your workspace role does not allow membership changes."}</p>
              <div className="shared-capabilities" aria-label="Your workspace capabilities">
                <strong>Your {capabilities?.role ?? workspace.role} access</strong>
                <span>Read evidence, retrieval, memory, exports, and activity.</span>
                <span>{canWrite ? "Create, edit, delete, and index shared evidence and memory." : "View-only: shared content cannot be changed."}</span>
                {canManageMembers ? <span>Manage workspace membership{canAssignAdmin ? " and assign administrator roles." : ", except administrator roles."}</span> : null}
                {canManageWorkspace ? <span>Rename or permanently delete this workspace.</span> : null}
              </div>
              <div className="shared-member-list">
                {members.map((member) => <article className={`shared-member-card role-${member.role}`} key={member.user.id}>
                  <UserAvatar className="shared-member-avatar" name={member.user.display_name} size={40} userId={member.user.id} />
                  <div><strong>{member.user.display_name}</strong><span>{member.user.email ?? "No email"}</span></div>
                  <span className="shared-member-role">{member.role}</span>
                  {canManageMembers && member.role !== "owner" ? <Button className="shared-member-remove" disabled={isSubmitting} onClick={() => void removeMember(member.user.id)} type="button" variant="secondary">Remove</Button> : null}
                </article>)}
              </div>
              {canManageMembers ? <section className="shared-settings-group">
                <p className="shared-muted-copy">Add a registered RepoMemo user or update their role. Administrators cannot create, edit, or remove other administrators; owners retain the final authority.</p>
                <form className="shared-member-form" onSubmit={saveMember}><Input aria-label="Member email" onChange={(event) => setMemberEmail(event.target.value)} placeholder="person@example.com" required type="email" value={memberEmail} /><Dropdown aria-label="Member role" onValueChange={(value) => setMemberRole(value as WorkspaceRole)} options={[{ label: "Viewer", value: "viewer" }, { label: "Member", value: "member" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} value={memberRole} /><Button disabled={isSubmitting} type="submit" variant="secondary">Add / update</Button></form>
              </section> : null}
              {canManageMembers ? <InviteMembers accessToken={accessToken} initialEmail={inviteEmail} key={`invite-${workspace.workspace.id}`} roleOptions={[{ label: "Member", value: "member" }, { label: "Viewer", value: "viewer" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} targetId={workspace.workspace.id} targetLabel="workspace" targetType="workspace" /> : null}
            </div> : null}
            {isActivityView ? <div className="shared-activity-section">
              <ContributionCalendar items={activityCalendar?.activity_by_day ?? []} title="Workspace activity" total={activityCalendar?.total_activity_count} />
              <div className="shared-panel-heading"><div><Timeline size={18} /><h2>Activity history</h2></div><span>{visibleActivity.length}</span></div>
              <div aria-label="Activity type" className="shared-activity-filter" role="group">
                {([["user", "User activity", userActivity.length], ["ai", "AI activity", aiActivity.length]] as const).map(([value, label, count]) => <Button aria-pressed={activityKind === value} className={activityKind === value ? "active" : ""} key={value} onClick={() => setActivityKind(value)} type="button" variant="secondary">{value === "ai" ? <Sparkles size={14} /> : <Users size={14} />}{label} <span>{count}</span></Button>)}
              </div>
              {visibleActivity.length ? <div className="shared-activity-list">{visibleActivity.map((event) => <article key={event.id}><div><strong>{event.summary}</strong><span>{activityKind === "ai" ? `Requested by ${event.actor?.display_name ?? "Unknown"}` : (event.actor?.display_name ?? "System")} · {event.action.replace(/_/g, " ")}</span></div><time dateTime={event.created_at}>{formatActivityTime(event.created_at)}</time></article>)}</div> : <div className="shared-empty-state">{activityKind === "ai" ? <Sparkles size={25} /> : <Timeline size={25} />}<strong>{activityKind === "ai" ? "No AI activity yet" : "No recorded activity yet"}</strong><span>{activityKind === "ai" ? "Assistant questions and generated answers will appear here." : "New workspace changes will appear here."}</span></div>}
            </div> : null}
          </aside> : null}
          {isSettingsView ? <section className="shared-settings-section">
            {canManageMembers ? <>
              <div className="shared-settings-policy"><Settings size={18} /><div><strong>Administrative workspace controls</strong><span>Changes are enforced by the shared API. Administrators can manage people and integrations; owner-only workspace actions are explicitly marked.</span></div></div>
              <AiProviderForm accessToken={accessToken} onSaved={onProviderSaved} providers={aiProviders} purpose="text" workspaceId={workspace.workspace.id} />
              <AiProviderForm accessToken={accessToken} onSaved={onProviderSaved} providers={aiProviders} purpose="vision" workspaceId={workspace.workspace.id} />
              <AiProviderForm accessToken={accessToken} onSaved={onProviderSaved} providers={aiProviders} purpose="embedding" status={embeddingStatus} workspaceId={workspace.workspace.id} />
              <AiPolicySettings accessToken={accessToken} key={`ai-policy-${workspace.workspace.id}`} onSaved={() => void load()} workspaceId={workspace.workspace.id} />
              <RepositorySettings accessToken={accessToken} key={`repositories-${workspace.workspace.id}`} onChanged={() => void load()} workspaceId={workspace.workspace.id} />
              {canManageWorkspace ? <section className="shared-settings-group">
                <div className="shared-panel-heading"><div><Settings size={18} /><h2>Workspace ownership</h2></div><span>owner only</span></div>
                <p className="shared-muted-copy">Rename this workspace or remove it permanently. Deletion removes its shared evidence, memory, and memberships.</p>
                <form className="shared-workspace-rename" onSubmit={renameWorkspace}><Input aria-label="Workspace name" onChange={(event) => setWorkspaceName(event.target.value)} required value={workspaceName} /><Button disabled={isSubmitting} type="submit" variant="secondary"><Pencil size={16} /> Rename</Button></form>
                <Button className="shared-danger-action" disabled={isSubmitting} onClick={() => void removeWorkspace()} type="button" variant="secondary"><Trash size={16} /> Delete workspace</Button>
              </section> : null}
            </> : <div className="shared-empty-state"><Settings size={25} /><strong>Administrative access required</strong><span>Only workspace administrators and the owner can open settings. Your current role can still browse shared evidence, memory, and tasks.</span></div>}
          </section> : null}
        </div> : null}
        </>}
      </section>
    </SharedLayout>
    </LiveEventsContext.Provider>
  );
}
