/*
THESIS: RepoMemo is a neutral research canvas, not an instrument panel; the working document and its evidence note share a quiet frame.
OWN-WORLD: White paper, near-black type, faint cool-gray seams, system type, 12px curves, and a single reserved blue for direct action or current selection.
STORY: Select a workspace, work in one broad primary canvas, and keep local context in a calm right-side note without scanning dashboards.
FIRST VIEWPORT: A 64px icon rail, a light title bar, a generous main task plane, and a 400px contextual note pane divided only by hairlines.
FORM: The user-pinned Rox productivity composition, translated to local project evidence with direct source facts and no decorative telemetry.
*/

import type { CSSProperties, FormEvent, ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { SharedWebApp } from "./SharedWebApp";
import {
  IconArchive as Archive,
  IconArrowRight as ArrowRight,
  IconBookmark as Bookmark,
  IconBook as BookOpen,
  IconBrain as Brain,
  IconChevronRight as ChevronRight,
  IconCircleCheck as CheckCircle2,
  IconClock as Clock,
  IconDatabase as Database,
  IconFileCode as FileCode2,
  IconPhoto as Photo,
  IconFolderDown as FolderDown,
  IconFolderPlus as FolderPlus,
  IconFilter as Filter,
  IconDeviceSdCard as HardDrive,
  IconStack2 as Layers3,
  IconLibrary as Library,
  IconLoader2 as Loader2,
  IconPlus as Plus,
  IconRefresh as RefreshCw,
  IconSearch as Search,
  IconSettings as Settings2,
  IconShieldLock as ShieldLock,
  IconWand as Wand2,
  IconUpload as Upload,
  IconX as X,
} from "@tabler/icons-react";
import {
  ACCEPTED_IMAGE_EXTENSIONS,
  ACCEPTED_TEXT_EXTENSIONS,
  chooseImportFiles,
  chooseImportFolder,
  createWorkspace,
  createMemoryCard,
  askWorkspace,
  embedWorkspace,
  getAppSettings,
  getArtifact,
  getMemoryCard,
  getWorkspaceOverview,
  indexArtifact,
  indexWorkspace,
  importPaths,
  importText,
  listArtifacts,
  listMemoryCards,
  listProviderSettings,
  listSymbols,
  listWorkspaces,
  PASTE_LANGUAGES,
  searchWorkspace,
  searchMemoryCards,
  searchSymbols,
  saveProviderSettings,
  summarizeArtifact,
  summarizeWorkspace,
  testProvider,
  exportMemoryCard,
  type PasteLanguage,
} from "./lib/repomemoApi";
import type {
  AppSettings,
  AskAnswer,
  ArtifactDetail,
  ArtifactSummary,
  IndexingJobStatus,
  ImportReport,
  MemoryCardDetail,
  MemoryCardSummary,
  SearchResult,
  ProviderSettings,
  ProviderTestResult,
  SummaryResult,
  Symbol,
  SymbolSearchResult,
  Workspace,
  WorkspaceOverview,
} from "./types";

type LoadState = "idle" | "loading" | "ready" | "error";
type View = "workspaces" | "import" | "artifacts" | "search" | "summary" | "ask" | "memory" | "settings";

const viewMeta: Record<View, { section: string; title: string }> = {
  workspaces: { section: "Library", title: "Workspaces" },
  import: { section: "Library", title: "Import sources" },
  artifacts: { section: "Library", title: "Artifacts" },
  search: { section: "Intelligence", title: "Search" },
  summary: { section: "Intelligence", title: "Project briefing" },
  ask: { section: "Intelligence", title: "Ask RepoMemo" },
  memory: { section: "Intelligence", title: "Memory cards" },
  settings: { section: "Preferences", title: "Provider settings" },
};

const emptyOverview = (workspaceId: string): WorkspaceOverview => ({
  workspace_id: workspaceId,
  source_count: 0,
  artifact_count: 0,
  chunk_count: 0,
  symbol_count: 0,
  memory_card_count: 0,
});

const isTauriRuntime = "__TAURI_INTERNALS__" in window;

export function App() {
  return isTauriRuntime ? <LocalDesktopApp /> : <SharedWebApp />;
}

function LocalDesktopApp() {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(
    null,
  );
  const [workspaceName, setWorkspaceName] = useState("");
  const [loadState, setLoadState] = useState<LoadState>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [activeView, setActiveView] = useState<View>("workspaces");
  const [artifacts, setArtifacts] = useState<ArtifactSummary[]>([]);
  const [selectedArtifactId, setSelectedArtifactId] = useState<string | null>(null);
  const [artifactDetail, setArtifactDetail] = useState<ArtifactDetail | null>(null);
  const [symbols, setSymbols] = useState<Symbol[]>([]);
  const [providers, setProviders] = useState<ProviderSettings[]>([]);
  const [providerTest, setProviderTest] = useState<ProviderTestResult | null>(null);
  const [isSavingProvider, setIsSavingProvider] = useState(false);
  const [isTestingProvider, setIsTestingProvider] = useState(false);
  const [summary, setSummary] = useState<SummaryResult | null>(null);
  const [workspaceSummary, setWorkspaceSummary] = useState<SummaryResult | null>(null);
  const [isSummarizing, setIsSummarizing] = useState(false);
  const [askQuestion, setAskQuestion] = useState("");
  const [askAnswer, setAskAnswer] = useState<AskAnswer | null>(null);
  const [memoryCards, setMemoryCards] = useState<MemoryCardSummary[]>([]);
  const [memorySearch, setMemorySearch] = useState("");
  const [selectedMemoryCardId, setSelectedMemoryCardId] = useState<string | null>(null);
  const [memoryCardDetail, setMemoryCardDetail] = useState<MemoryCardDetail | null>(null);
  const [memoryTitle, setMemoryTitle] = useState("");
  const [memoryBody, setMemoryBody] = useState("");
  const [isSavingMemory, setIsSavingMemory] = useState(false);
  const [isAsking, setIsAsking] = useState(false);
  const [isEmbedding, setIsEmbedding] = useState(false);
  const [overview, setOverview] = useState<WorkspaceOverview | null>(null);
  const [importReport, setImportReport] = useState<ImportReport | null>(null);
  const [indexingJob, setIndexingJob] = useState<IndexingJobStatus | null>(null);
  const [isIndexing, setIsIndexing] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [pasteTitle, setPasteTitle] = useState("");
  const [pasteContent, setPasteContent] = useState("");
  const [pasteLanguage, setPasteLanguage] = useState<PasteLanguage>("Text");
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [selectedSearchResult, setSelectedSearchResult] = useState<SearchResult | null>(null);
  const [symbolSearchResults, setSymbolSearchResults] = useState<SymbolSearchResult[]>([]);
  const [selectedSymbolResult, setSelectedSymbolResult] = useState<SymbolSearchResult | null>(null);
  const [searchLanguages, setSearchLanguages] = useState<string[]>([]);
  const [searchTypes, setSearchTypes] = useState<ArtifactSummary["artifact_type"][]>([]);
  const [searchSources, setSearchSources] = useState<string[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [hasSearched, setHasSearched] = useState(false);
  useEffect(() => {
    let cancelled = false;

    async function boot() {
      setLoadState("loading");
      setErrorMessage(null);

      try {
        const [workspaceList, appSettings] = await Promise.all([
          listWorkspaces(),
          getAppSettings(),
        ]);

        if (cancelled) {
          return;
        }

        setWorkspaces(workspaceList);
        setSettings(appSettings);
        setSelectedWorkspaceId(workspaceList[0]?.id ?? null);
        setLoadState("ready");
      } catch (error) {
        if (cancelled) {
          return;
        }

        setLoadState("error");
        setErrorMessage(error instanceof Error ? error.message : String(error));
      }
    }

    boot();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!selectedWorkspaceId) {
      setArtifacts([]);
      setOverview(null);
      setSelectedArtifactId(null);
      setProviders([]);
      setMemoryCards([]);
      setSelectedMemoryCardId(null);
      setMemoryCardDetail(null);
      return;
    }

    refreshWorkspaceData(selectedWorkspaceId);
    refreshMemoryCards(selectedWorkspaceId);
    listProviderSettings(selectedWorkspaceId).then(setProviders).catch((error) => {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    });
  }, [selectedWorkspaceId]);

  useEffect(() => {
    let cancelled = false;

    async function loadArtifact() {
      if (!selectedArtifactId) {
        setArtifactDetail(null);
        setSymbols([]);
        return;
      }

      try {
        const [detail, artifactSymbols] = await Promise.all([
          getArtifact(selectedArtifactId),
          listSymbols(selectedArtifactId),
        ]);
        if (!cancelled) {
          setArtifactDetail(detail);
          setSymbols(artifactSymbols);
        }
      } catch (error) {
        if (!cancelled) {
          setErrorMessage(error instanceof Error ? error.message : String(error));
        }
      }
    }

    loadArtifact();

    return () => {
      cancelled = true;
    };
  }, [selectedArtifactId]);

  useEffect(() => {
    let cancelled = false;
    if (!selectedMemoryCardId) {
      setMemoryCardDetail(null);
      return;
    }
    getMemoryCard(selectedMemoryCardId)
      .then((detail) => { if (!cancelled) setMemoryCardDetail(detail); })
      .catch((error) => { if (!cancelled) setErrorMessage(error instanceof Error ? error.message : String(error)); });
    return () => { cancelled = true; };
  }, [selectedMemoryCardId]);

  const selectedWorkspace = useMemo(
    () => workspaces.find((workspace) => workspace.id === selectedWorkspaceId),
    [selectedWorkspaceId, workspaces],
  );

  const currentOverview = overview ?? emptyOverview(selectedWorkspaceId ?? "");
  const activeProvider = providers.find((provider) => provider.enabled);
  const indexedArtifactCount = artifacts.filter((artifact) => Boolean(artifact.indexed_at)).length;
  const artifactCoverage = artifacts.length > 0
    ? Math.round((indexedArtifactCount / artifacts.length) * 100)
    : 0;
  const indexingProgress = indexingJob?.progress_total
    ? Math.min(100, Math.round((indexingJob.progress_current / indexingJob.progress_total) * 100))
    : null;
  const normalizedIndexingStatus = indexingJob?.status.toLowerCase() ?? "";
  const indexingFailed = normalizedIndexingStatus === "failed" || normalizedIndexingStatus === "error";
  const indexingActive = isIndexing || Boolean(
    indexingJob
    && !["completed", "failed", "error", "cancelled"].includes(normalizedIndexingStatus),
  );
  const indexingTone = indexingFailed
    ? "error"
    : indexingActive
      ? "attention"
      : artifacts.length === 0
        ? "waiting"
        : indexedArtifactCount === artifacts.length
          ? "ready"
          : indexedArtifactCount > 0
            ? "partial"
            : "waiting";
  const displayedIndexingProgress = indexingActive && indexingProgress !== null
    ? indexingProgress
    : artifactCoverage;
  const indexingLabel = indexingFailed
    ? "Failed"
    : indexingActive
      ? indexingJob?.stage || "Indexing"
      : artifacts.length === 0
        ? "Waiting"
        : indexedArtifactCount === artifacts.length
          ? "Complete"
          : indexedArtifactCount > 0
            ? "Partial"
            : "Waiting";

  async function refreshWorkspaceData(workspaceId = selectedWorkspaceId) {
    if (!workspaceId) {
      return;
    }

    try {
      const [artifactList, workspaceOverview] = await Promise.all([
        listArtifacts(workspaceId),
        getWorkspaceOverview(workspaceId),
      ]);
      setArtifacts(artifactList);
      setOverview(workspaceOverview);
      setSelectedArtifactId((current) =>
        artifactList.some((artifact) => artifact.id === current)
          ? current
          : artifactList[0]?.id ?? null,
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function refreshMemoryCards(workspaceId = selectedWorkspaceId, query = memorySearch) {
    if (!workspaceId) return;
    try {
      const cards = query.trim()
        ? await searchMemoryCards(workspaceId, query)
        : await listMemoryCards(workspaceId);
      setMemoryCards(cards);
      setSelectedMemoryCardId((current) => cards.some((card) => card.id === current) ? current : cards[0]?.id ?? null);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleSaveMemory(
    title: string,
    bodyMarkdown: string,
    source: string,
    citations: SummaryResult["citations"] = [],
    confidence: number | null = null,
  ) {
    if (!selectedWorkspaceId || !title.trim() || !bodyMarkdown.trim()) return;
    setIsSavingMemory(true);
    setErrorMessage(null);
    try {
      const card = await createMemoryCard({
        workspace_id: selectedWorkspaceId,
        title: title.trim(),
        body_markdown: bodyMarkdown.trim(),
        source,
        confidence,
        citations,
      });
      setMemoryTitle("");
      setMemoryBody("");
      setMemorySearch("");
      await Promise.all([refreshMemoryCards(selectedWorkspaceId, ""), refreshWorkspaceData(selectedWorkspaceId)]);
      setSelectedMemoryCardId(card.id);
      setActiveView("memory");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingMemory(false);
    }
  }

  async function handleMemorySearch(query: string) {
    setMemorySearch(query);
    await refreshMemoryCards(selectedWorkspaceId, query);
  }

  async function handleExportMemory(cardId: string) {
    try {
      const markdown = await exportMemoryCard(cardId);
      const href = URL.createObjectURL(new Blob([markdown], { type: "text/markdown" }));
      const anchor = document.createElement("a");
      anchor.href = href;
      anchor.download = `${memoryCardDetail?.card.title ?? "repomemo-memory"}.md`.replace(/[\\/:*?"<>|]/g, "-");
      anchor.click();
      URL.revokeObjectURL(href);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleCreateWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const name = workspaceName.trim();
    if (!name) {
      return;
    }

    setErrorMessage(null);

    try {
      const workspace = await createWorkspace(name);
      setWorkspaces((current) => [workspace, ...current]);
      setSelectedWorkspaceId(workspace.id);
      setWorkspaceName("");
      setActiveView("import");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleImportFiles() {
    const paths = await chooseImportFiles();
    await runImport(paths);
  }

  async function handleImportFolder() {
    const paths = await chooseImportFolder();
    await runImport(paths);
  }

  async function handlePasteImport(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedWorkspaceId || !pasteContent.trim()) {
      return;
    }

    setIsImporting(true);
    setErrorMessage(null);

    try {
      const artifact = await importText(
        selectedWorkspaceId,
        pasteTitle.trim(),
        pasteContent,
        pasteLanguage,
      );
      setImportReport({
        workspace_id: selectedWorkspaceId,
        scanned: 1,
        imported: 1,
        duplicates: 0,
        skipped: 0,
        failed: 0,
        imported_artifacts: [artifact],
        skipped_items: [],
      });
      await refreshWorkspaceData(selectedWorkspaceId);
      setPasteTitle("");
      setPasteContent("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsImporting(false);
    }
  }

  async function runImport(paths: string[]) {
    if (!selectedWorkspaceId || paths.length === 0) {
      return;
    }

    setIsImporting(true);
    setErrorMessage(null);

    try {
      const report = await importPaths(selectedWorkspaceId, paths);
      setImportReport(report);
      await refreshWorkspaceData(selectedWorkspaceId);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsImporting(false);
    }
  }

  async function handleIndexArtifact(artifactId = selectedArtifactId) {
    if (!artifactId) {
      return;
    }

    setIsIndexing(true);
    setErrorMessage(null);

    try {
      const job = await indexArtifact(artifactId);
      setIndexingJob(job);
      await refreshWorkspaceData(selectedWorkspaceId);
      const detail = await getArtifact(artifactId);
      setArtifactDetail(detail);
      setSymbols(await listSymbols(artifactId));
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsIndexing(false);
    }
  }

  async function handleIndexWorkspace() {
    if (!selectedWorkspaceId) {
      return;
    }

    setIsIndexing(true);
    setErrorMessage(null);

    try {
      const job = await indexWorkspace(selectedWorkspaceId);
      setIndexingJob(job);
      await refreshWorkspaceData(selectedWorkspaceId);
      if (selectedArtifactId) {
        const detail = await getArtifact(selectedArtifactId);
        setArtifactDetail(detail);
        setSymbols(await listSymbols(selectedArtifactId));
      }
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsIndexing(false);
    }
  }

  async function handleSearch(event?: FormEvent<HTMLFormElement>) {
    event?.preventDefault();
    if (!selectedWorkspaceId || !searchQuery.trim()) {
      setSearchResults([]);
      setSymbolSearchResults([]);
      setSelectedSearchResult(null);
      setSelectedSymbolResult(null);
      setHasSearched(false);
      return;
    }

    setIsSearching(true);
    setErrorMessage(null);
    try {
      const [results, symbolResults] = await Promise.all([
        searchWorkspace({
          workspace_id: selectedWorkspaceId,
          query: searchQuery.trim(),
          artifact_types: searchTypes,
          languages: searchLanguages,
          source_ids: searchSources,
          limit: 40,
        }),
        searchSymbols(selectedWorkspaceId, searchQuery.trim()),
      ]);
      const filteredSymbolResults = symbolResults.filter((result) => {
        const artifact = artifacts.find((item) => item.id === result.symbol.artifact_id);
        if (!artifact) return false;
        if (searchTypes.length > 0 && !searchTypes.includes(artifact.artifact_type)) return false;
        if (searchLanguages.length > 0 && (!artifact.language || !searchLanguages.includes(artifact.language))) return false;
        if (searchSources.length > 0 && !searchSources.includes(artifact.source_id)) return false;
        return true;
      });
      setSearchResults(results);
      setSymbolSearchResults(filteredSymbolResults);
      setSelectedSymbolResult(filteredSymbolResults[0] ?? null);
      setSelectedSearchResult(filteredSymbolResults.length === 0 ? (results[0] ?? null) : null);
      setHasSearched(true);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSearching(false);
    }
  }

  function openSearchArtifact(result: SearchResult) {
    setSelectedArtifactId(result.artifact_id);
    setActiveView("artifacts");
  }

  function openSymbolArtifact(result: SymbolSearchResult) {
    setSelectedArtifactId(result.symbol.artifact_id);
    setActiveView("artifacts");
  }

  async function handleSaveProvider(settingsToSave: ProviderSettings) {
    setIsSavingProvider(true);
    setErrorMessage(null);
    try {
      const saved = await saveProviderSettings(settingsToSave);
      setProviders(await listProviderSettings(saved.workspace_id ?? selectedWorkspaceId ?? ""));
      setSettings(await getAppSettings());
      setProviderTest(null);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingProvider(false);
    }
  }

  async function handleTestProvider(providerId: string) {
    setIsTestingProvider(true);
    setErrorMessage(null);
    try {
      setProviderTest(await testProvider(providerId));
    } catch (error) {
      setProviderTest({ provider_id: providerId, success: false, message: error instanceof Error ? error.message : String(error) });
    } finally {
      setIsTestingProvider(false);
    }
  }

  async function handleSummarize(artifactId: string) {
    const provider = providers.find((item) => item.enabled);
    if (!provider) {
      setActiveView("settings");
      setErrorMessage("Enable a provider before requesting a summary. No content was sent.");
      return;
    }
    setIsSummarizing(true);
    setErrorMessage(null);
    try {
      setSummary(await summarizeArtifact(artifactId, provider.id));
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSummarizing(false);
    }
  }

  async function handleWorkspaceSummary() {
    const provider = providers.find((item) => item.enabled);
    if (!provider || !selectedWorkspaceId) {
      setActiveView("settings");
      setErrorMessage("Enable a provider before requesting a workspace summary. No content was sent.");
      return;
    }
    setIsSummarizing(true);
    setErrorMessage(null);
    try {
      setWorkspaceSummary(await summarizeWorkspace(selectedWorkspaceId, provider.id));
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSummarizing(false);
    }
  }

  async function handleAsk() {
    const provider = providers.find((item) => item.enabled);
    if (!provider || !selectedWorkspaceId || !askQuestion.trim()) {
      if (!provider) { setActiveView("settings"); setErrorMessage("Enable a provider before using Ask. No content was sent."); }
      return;
    }
    setIsAsking(true);
    setErrorMessage(null);
    try {
      setAskAnswer(await askWorkspace({ workspace_id: selectedWorkspaceId, question: askQuestion.trim(), provider_id: provider.id, limit: 10 }));
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally { setIsAsking(false); }
  }

  async function handleEmbedWorkspace() {
    const provider = providers.find((item) => item.enabled);
    if (!provider || !selectedWorkspaceId) { setActiveView("settings"); setErrorMessage("Enable a local embedding provider first."); return; }
    setIsEmbedding(true); setErrorMessage(null);
    try { setIndexingJob(await embedWorkspace(selectedWorkspaceId, provider.id)); await refreshWorkspaceData(selectedWorkspaceId); }
    catch (error) { setErrorMessage(error instanceof Error ? error.message : String(error)); }
    finally { setIsEmbedding(false); }
  }

  function openNextSafeAction() {
    if (!selectedWorkspace) {
      setActiveView("workspaces");
      requestAnimationFrame(() => document.getElementById("workspace-name")?.focus());
      return;
    }

    if (currentOverview.source_count === 0) {
      setActiveView("import");
      return;
    }

    if (artifacts.length === 0 || indexedArtifactCount < artifacts.length) {
      setActiveView("artifacts");
      return;
    }

    setActiveView("search");
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand-mark">
          <div className="brand-glyph" aria-hidden="true">
            <Layers3 size={22} stroke={1.9} />
          </div>
          <div>
            <p className="meta-label">Local memory</p>
            <h1>RepoMemo</h1>
          </div>
        </div>

        <nav className="nav-list" aria-label="Primary">
          <div className="nav-group">
            <p className="nav-cluster-label">Library</p>
            <NavButton
              active={activeView === "workspaces"}
              icon={Library}
              label="Workspaces"
              onClick={() => setActiveView("workspaces")}
            />
            <NavButton
              active={activeView === "import"}
              icon={FolderDown}
              label="Import"
              onClick={() => setActiveView("import")}
              disabled={!selectedWorkspace}
            />
            <NavButton
              active={activeView === "artifacts"}
              icon={Archive}
              label="Artifacts"
              onClick={() => setActiveView("artifacts")}
              disabled={!selectedWorkspace}
            />
          </div>
          <div className="nav-divider" aria-hidden="true" />
          <div className="nav-group">
            <p className="nav-cluster-label">Intelligence</p>
            <NavButton
              active={activeView === "search"}
              disabled={!selectedWorkspace}
              icon={Search}
              label="Search"
              onClick={() => setActiveView("search")}
            />
            <NavButton
              active={activeView === "summary"}
              disabled={!selectedWorkspace}
              icon={Wand2}
              label="Summary"
              onClick={() => setActiveView("summary")}
            />
            <NavButton active={activeView === "ask"} disabled={!selectedWorkspace} icon={Brain} label="Ask" onClick={() => setActiveView("ask")} />
            <NavButton active={activeView === "memory"} disabled={!selectedWorkspace} icon={Bookmark} label="Memory cards" onClick={() => setActiveView("memory")} />
          </div>
          <div className="nav-divider" aria-hidden="true" />
          <div className="nav-group">
            <NavButton
              active={activeView === "settings"}
              disabled={!selectedWorkspace}
              icon={Settings2}
              label="Settings"
              onClick={() => setActiveView("settings")}
            />
          </div>
        </nav>

        <section className="storage-panel" aria-label="Storage settings">
          <div className="panel-icon" aria-hidden="true">
            <HardDrive size={17} />
          </div>
          <div>
            <p className="panel-label">Storage root</p>
            <p className="storage-path">{settings?.data_dir ?? "Loading..."}</p>
          </div>
        </section>
      </aside>

      <section className="workbench">
        <header className="workbench-header workspace-header-v3 graphite-header">
          {activeView === "workspaces" ? (
            <>
              <div className="workspace-header-copy">
                <p className="workspace-breadcrumb"><span>Library</span><ChevronRight size={13} /> Workspaces</p>
                <div className="workspace-heading-line">
                  <h2>Workspaces</h2>
                  <span>{workspaces.length}</span>
                </div>
              </div>
              <div className="workspace-header-actions">
                <div className="workspace-runtime-status" aria-live="polite">
                  <span className={`workspace-runtime-dot ${loadState === "error" ? "error" : loadState === "ready" ? "ready" : "busy"}`} aria-hidden="true" />
                  <span>
                    <strong>{loadState === "error" ? "Local core unavailable" : loadState === "ready" ? "Local database ready" : "Starting local core"}</strong>
                    <small>Private on this device</small>
                  </span>
                </div>
                <button
                  className={`workspace-new-button ${selectedWorkspace ? "secondary" : "primary"}`}
                  type="button"
                  onClick={() => document.getElementById("workspace-name")?.focus()}
                >
                  <Plus size={16} /> New workspace
                </button>
              </div>
            </>
          ) : (
            <>
              <div className="workspace-header-copy">
            <p className="workspace-breadcrumb"><span>{viewMeta[activeView].section}</span><ChevronRight size={13} /> {viewMeta[activeView].title}</p>
            <h2>{viewMeta[activeView].title}</h2>
          </div>
          <div className="header-actions workspace-header-actions">
            {selectedWorkspace ? <div className="workspace-context-chip"><Layers3 size={14} /><span>{selectedWorkspace.name}</span></div> : null}
            <StatusBadge tone={loadState === "error" ? "danger" : loadState === "ready" ? "success" : "warning"}>
              {loadState === "error" ? <X size={15} /> : loadState === "ready" ? <CheckCircle2 size={15} /> : <Loader2 className="spin" size={15} />}
              {loadState === "error" ? "Local core unavailable" : loadState === "ready" ? "Local core ready" : "Booting core"}
            </StatusBadge>
            <StatusBadge tone={indexingTone === "error" ? "danger" : indexingTone === "attention" ? "warning" : indexingTone === "ready" ? "success" : "neutral"}>
              <RefreshCw className={indexingActive ? "spin" : undefined} size={14} />
              Index {indexingLabel}
            </StatusBadge>
            <StatusBadge tone={settings?.ai_enabled ? "success" : "neutral"}>
              {settings?.ai_enabled ? `${providers.find((item) => item.enabled)?.provider_type === "openrouter" ? "Cloud" : "Local"} · ${settings.active_provider}` : "No AI"}
            </StatusBadge>
            <StatusBadge tone={activeProvider ? "success" : "neutral"}>
              {activeProvider ? `${activeProvider.provider_type === "openrouter" ? "Cloud" : "Local"} AI` : "AI off"}
            </StatusBadge>
              </div>
            </>
          )}
        </header>

        {errorMessage ? <div className="error-banner">{errorMessage}</div> : null}

        {activeView === "workspaces" ? (
          <WorkspaceView
            currentOverview={currentOverview}
            indexedArtifactCount={indexedArtifactCount}
            onCreateWorkspace={handleCreateWorkspace}
            onNavigate={(view) => setActiveView(view)}
            onSelectWorkspace={setSelectedWorkspaceId}
            selectedWorkspaceId={selectedWorkspaceId}
            workspaceName={workspaceName}
            workspaces={workspaces}
            setWorkspaceName={setWorkspaceName}
          />
        ) : activeView === "import" ? (
          <ImportView
            artifacts={artifacts}
            importReport={importReport}
            isImporting={isImporting}
            onImportFiles={handleImportFiles}
            onImportFolder={handleImportFolder}
            onPasteImport={handlePasteImport}
            overview={currentOverview}
            pasteContent={pasteContent}
            pasteLanguage={pasteLanguage}
            pasteTitle={pasteTitle}
            selectedWorkspace={selectedWorkspace}
            setPasteContent={setPasteContent}
            setPasteLanguage={setPasteLanguage}
            setPasteTitle={setPasteTitle}
          />
        ) : activeView === "artifacts" ? (
          <ArtifactsView
            artifactDetail={artifactDetail}
            artifacts={artifacts}
            indexingJob={indexingJob}
            isIndexing={isIndexing}
            onIndexArtifact={handleIndexArtifact}
            onIndexWorkspace={handleIndexWorkspace}
            onRefresh={() => refreshWorkspaceData()}
            onSelectArtifact={setSelectedArtifactId}
            overview={currentOverview}
            selectedArtifactId={selectedArtifactId}
            selectedWorkspace={selectedWorkspace}
            symbols={symbols}
            summary={summary}
            isSummarizing={isSummarizing}
            onSummarize={handleSummarize}
            onSaveMemory={handleSaveMemory}
          />
        ) : activeView === "search" ? (
          <SearchView
            artifacts={artifacts}
            hasSearched={hasSearched}
            isSearching={isSearching}
            languages={searchLanguages}
            onOpenArtifact={openSearchArtifact}
            onOpenSymbol={openSymbolArtifact}
            onSearch={handleSearch}
            onSelectResult={(result) => { setSelectedSearchResult(result); setSelectedSymbolResult(null); }}
            onSelectSymbol={(result) => { setSelectedSymbolResult(result); setSelectedSearchResult(null); }}
            query={searchQuery}
            results={searchResults}
            selectedResult={selectedSearchResult}
            selectedSymbol={selectedSymbolResult}
            setLanguages={setSearchLanguages}
            setQuery={setSearchQuery}
            setSources={setSearchSources}
            setTypes={setSearchTypes}
            sources={searchSources}
            symbolResults={symbolSearchResults}
            types={searchTypes}
            workspace={selectedWorkspace}
          />
        ) : activeView === "summary" ? (
          <WorkspaceSummaryView
            isSummarizing={isSummarizing}
            onSummarize={handleWorkspaceSummary}
            onSaveMemory={handleSaveMemory}
            summary={workspaceSummary}
            workspace={selectedWorkspace}
          />
        ) : activeView === "ask" ? (
          <AskView askAnswer={askAnswer} isAsking={isAsking} isEmbedding={isEmbedding} onAsk={handleAsk} onEmbed={handleEmbedWorkspace} onSaveMemory={handleSaveMemory} question={askQuestion} setQuestion={setAskQuestion} workspace={selectedWorkspace} />
        ) : activeView === "memory" ? (
          <MemoryCardsView
            cards={memoryCards}
            detail={memoryCardDetail}
            isSaving={isSavingMemory}
            onCreate={() => handleSaveMemory(memoryTitle, memoryBody, "manual")}
            onExport={handleExportMemory}
            onSearch={handleMemorySearch}
            onSelect={setSelectedMemoryCardId}
            searchQuery={memorySearch}
            selectedCardId={selectedMemoryCardId}
            setBody={setMemoryBody}
            setTitle={setMemoryTitle}
            title={memoryTitle}
            body={memoryBody}
            workspace={selectedWorkspace}
          />
        ) : (
          <ProviderSettingsView
            isSaving={isSavingProvider}
            isTesting={isTestingProvider}
            onSave={handleSaveProvider}
            onTest={handleTestProvider}
            provider={providers[0] ?? null}
            testResult={providerTest}
            workspace={selectedWorkspace}
          />
        )}
      </section>

      <aside className="status-spine" aria-label="System status and next action">
        <header className="status-spine-header">
          <div>
            <span className="instrument-index">SYS</span>
            <strong>Local boundary</strong>
          </div>
          <span
            className={`state-lamp ${loadState === "error" ? "error" : loadState === "ready" ? "ready" : "busy"}`}
            aria-label={loadState === "error" ? "Local core unavailable" : loadState === "ready" ? "Local core ready" : "Local core starting"}
          />
        </header>

        <section className="status-module">
          <div className="status-module-title">
            <Database size={15} aria-hidden="true" />
            <span>Local database</span>
            <strong>{loadState === "error" ? "Unavailable" : loadState === "ready" ? "Ready" : "Starting"}</strong>
          </div>
          <dl>
            <div><dt>Scope</dt><dd>{selectedWorkspace ? selectedWorkspace.name : "No workspace"}</dd></div>
            <div><dt>Sources</dt><dd>{currentOverview.source_count}</dd></div>
            <div><dt>Artifacts</dt><dd>{currentOverview.artifact_count}</dd></div>
          </dl>
        </section>

        <section className="status-module">
          <div className="status-module-title">
            <HardDrive size={15} aria-hidden="true" />
            <span>Storage root</span>
            <strong>Local</strong>
          </div>
          <p className="status-path" title={settings?.data_dir}>{settings?.data_dir ?? "Resolving local path…"}</p>
          <dl>
            <div><dt>Chunks</dt><dd>{currentOverview.chunk_count}</dd></div>
            <div><dt>Symbols</dt><dd>{currentOverview.symbol_count}</dd></div>
          </dl>
        </section>

        <section className={`status-module ${indexingTone}`}>
          <div className="status-module-title">
            <RefreshCw size={15} aria-hidden="true" />
            <span>Indexing</span>
            <strong>{indexingLabel}</strong>
          </div>
          <div
            className="instrument-progress"
            role="progressbar"
            aria-label="Indexing progress"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={displayedIndexingProgress}
            aria-valuetext={indexingFailed
              ? "Indexing failed"
              : `${indexedArtifactCount} of ${artifacts.length} artifacts indexed`}
          >
            <span style={{ "--progress": `${displayedIndexingProgress}%` } as CSSProperties} />
          </div>
          <dl>
            <div>
              <dt>Coverage</dt>
              <dd>
                {indexingFailed
                  ? "Failed"
                  : indexingActive && indexingProgress !== null
                    ? `${indexingProgress}%`
                    : artifacts.length === 0
                      ? "Not started"
                      : `${indexedArtifactCount}/${artifacts.length} artifacts`}
              </dd>
            </div>
            <div><dt>Memory</dt><dd>{currentOverview.memory_card_count}</dd></div>
          </dl>
          {indexingFailed && indexingJob?.error_message
            ? <p className="status-module-copy status-error-copy">{indexingJob.error_message}</p>
            : null}
        </section>

        <section className="status-module">
          <div className="status-module-title">
            <Brain size={15} aria-hidden="true" />
            <span>AI boundary</span>
            <strong>{activeProvider ? (activeProvider.provider_type === "openrouter" ? "Cloud" : "Local") : "Off"}</strong>
          </div>
          <p className="status-module-copy">
            {activeProvider ? `${activeProvider.name}${activeProvider.model ? ` · ${activeProvider.model}` : ""}` : "Search and artifact inspection remain available without AI."}
          </p>
          <button
            className="status-text-action"
            disabled={!selectedWorkspace}
            onClick={() => setActiveView("settings")}
            type="button"
          >
            Configure provider <ArrowRight size={13} />
          </button>
        </section>

        <section className="status-module next-safe-action">
          <span className="instrument-index">NEXT SAFE ACTION</span>
          <strong>
            {!selectedWorkspace
              ? "Create a workspace"
              : currentOverview.source_count === 0
                ? "Import first source"
                : artifacts.length === 0 || indexedArtifactCount < artifacts.length
                  ? "Inspect and index"
                  : "Search local evidence"}
          </strong>
          <button className="spine-primary-action" onClick={openNextSafeAction} type="button">
            {!selectedWorkspace
              ? <Plus size={15} />
              : currentOverview.source_count === 0
                ? <FolderDown size={15} />
                : artifacts.length === 0 || indexedArtifactCount < artifacts.length
                  ? <Archive size={15} />
                  : <Search size={15} />}
            Continue
            <ArrowRight size={14} />
          </button>
        </section>

        <footer className="status-spine-footer">
          <span className="state-lamp" aria-hidden="true" />
          Local-first
          <span>v0.1</span>
        </footer>
      </aside>
    </main>
  );
}

function WorkspaceView({
  currentOverview,
  indexedArtifactCount,
  onCreateWorkspace,
  onNavigate,
  onSelectWorkspace,
  selectedWorkspaceId,
  setWorkspaceName,
  workspaceName,
  workspaces,
}: {
  currentOverview: WorkspaceOverview;
  indexedArtifactCount: number;
  onCreateWorkspace: (event: FormEvent<HTMLFormElement>) => void;
  onNavigate: (view: "import" | "artifacts" | "search") => void;
  onSelectWorkspace: (id: string) => void;
  selectedWorkspaceId: string | null;
  setWorkspaceName: (value: string) => void;
  workspaceName: string;
  workspaces: Workspace[];
}) {
  const selectedWorkspace = workspaces.find((workspace) => workspace.id === selectedWorkspaceId);
  const hasArtifacts = currentOverview.artifact_count > 0;
  const hasIndexedArtifacts = indexedArtifactCount > 0;
  const allArtifactsIndexed = hasArtifacts
    && indexedArtifactCount === currentOverview.artifact_count;
  return (
    <div className="workspace-page-v3">
      <section className="workspace-directory" aria-labelledby="workspace-directory-title">
        <header className="workspace-directory-header">
          <div>
            <p className="workspace-kicker">Local library</p>
            <h3 id="workspace-directory-title">Your workspaces</h3>
            <p>Select a workspace to inspect its local knowledge map and continue working.</p>
          </div>
          <span className="workspace-count">{workspaces.length} total</span>
        </header>

        <div className="workspace-list-v3" aria-live="polite">
          {workspaces.length === 0 ? (
            <div className="workspace-empty-v3">
              <Library size={22} />
              <strong>No workspaces yet</strong>
              <p>Create a private workspace below, then add a repository or a set of project documents.</p>
            </div>
          ) : (
            workspaces.map((workspace) => (
              <button
                className={workspace.id === selectedWorkspaceId ? "workspace-row-v3 selected" : "workspace-row-v3"}
                aria-pressed={workspace.id === selectedWorkspaceId}
                key={workspace.id}
                type="button"
                onClick={() => onSelectWorkspace(workspace.id)}
              >
                <span className="workspace-row-mark" aria-hidden="true"><Layers3 size={18} /></span>
                <span className="workspace-row-copy">
                  <strong>{workspace.name}</strong>
                  <small><ShieldLock size={12} /> Local workspace</small>
                </span>
                <span className="workspace-row-time" title={formatDate(workspace.updated_at)}>
                  <Clock size={13} /> {formatRelativeDate(workspace.updated_at)}
                  <ChevronRight size={16} aria-hidden="true" />
                </span>
              </button>
            ))
          )}
        </div>

        <form className="workspace-create-v3" onSubmit={onCreateWorkspace}>
          <div className="workspace-create-heading">
            <span aria-hidden="true"><Plus size={16} /></span>
            <label htmlFor="workspace-name">
              <strong>Create a workspace</strong>
              <small>Everything starts local and stays under your control.</small>
            </label>
          </div>
          <div className="workspace-create-controls">
            <input
              id="workspace-name"
              value={workspaceName}
              onChange={(event) => setWorkspaceName(event.target.value)}
              placeholder="e.g. Payments platform"
              autoComplete="off"
            />
            <button className="workspace-create-button" disabled={!workspaceName.trim()} type="submit">
              Create <ArrowRight size={15} />
            </button>
          </div>
        </form>
      </section>

      <aside className="workspace-inspector-v3" aria-label="Selected workspace details">
        {selectedWorkspace ? (
          <>
            <header className="workspace-inspector-header">
              <div className="workspace-local-badge"><ShieldLock size={14} /> Stored locally</div>
              <p className="workspace-selection-meta">Selected workspace <span aria-hidden="true">/</span> Updated {formatRelativeDate(selectedWorkspace.updated_at)}</p>
              <h3>{selectedWorkspace.name}</h3>
              <p>Project artifacts, searchable context, and durable memory in one private knowledge boundary.</p>
            </header>

            {false ? <section className="workspace-evidence-path" aria-labelledby="workspace-evidence-title">
              <div className="workspace-section-heading">
                <div>
                  <p className="workspace-kicker">Evidence path</p>
                  <h4 id="workspace-evidence-title">Local knowledge flow</h4>
                </div>
                <span>Live totals</span>
              </div>
              <div className="workspace-trace-canvas">
                <div className="workspace-trace-stack input">
                  <div className={currentOverview.source_count > 0 ? "workspace-trace-node ready" : "workspace-trace-node"}>
                    <span><FolderDown size={15} /></span>
                    <strong>Sources</strong>
                    <small>{currentOverview.source_count}</small>
                  </div>
                  <div className={currentOverview.artifact_count > 0 ? "workspace-trace-node ready" : "workspace-trace-node"}>
                    <span><Archive size={15} /></span>
                    <strong>Artifacts</strong>
                    <small>{currentOverview.artifact_count}</small>
                  </div>
                </div>
                <div className="workspace-trace-bridge input" aria-hidden="true">
                  <i /><i /><b />
                </div>
                <div className={hasIndexedArtifacts ? "workspace-trace-node core ready" : "workspace-trace-node core"}>
                  <span><Database size={17} /></span>
                  <strong>Indexed evidence</strong>
                  <small>{indexedArtifactCount}/{currentOverview.artifact_count} artifacts · {currentOverview.chunk_count} chunks</small>
                </div>
                <div className="workspace-trace-bridge output" aria-hidden="true">
                  <b /><i /><i />
                </div>
                <div className="workspace-trace-stack output">
                  <div className={currentOverview.symbol_count > 0 ? "workspace-trace-node ready" : "workspace-trace-node"}>
                    <span><FileCode2 size={15} /></span>
                    <strong>Symbols</strong>
                    <small>{currentOverview.symbol_count}</small>
                  </div>
                  <div className={currentOverview.memory_card_count > 0 ? "workspace-trace-node ready" : "workspace-trace-node"}>
                    <span><Brain size={15} /></span>
                    <strong>Memory</strong>
                    <small>{currentOverview.memory_card_count}</small>
                  </div>
                </div>
              </div>
              <p className="workspace-trace-caption">
                <span className="state-lamp ready" aria-hidden="true" />
                Every derived result stays registered to local source material.
              </p>
            </section> : null}

            {false ? <dl className="workspace-metrics-v3" aria-label="Workspace totals">
              <div><dt>Sources</dt><dd>{currentOverview.source_count}</dd></div>
              <div><dt>Artifacts</dt><dd>{currentOverview.artifact_count}</dd></div>
              <div><dt>Chunks</dt><dd>{currentOverview.chunk_count}</dd></div>
              <div><dt>Symbols</dt><dd>{currentOverview.symbol_count}</dd></div>
              <div><dt>Memory</dt><dd>{currentOverview.memory_card_count}</dd></div>
            </dl> : null}

            {false ? <section className="workspace-pipeline" aria-labelledby="workspace-pipeline-title">
              <div className="workspace-section-heading">
                <div>
                  <p className="workspace-kicker">Knowledge pipeline</p>
                  <h4 id="workspace-pipeline-title">Workspace readiness</h4>
                </div>
                <span>0 of 0</span>
              </div>
              <ol />
            </section> : null}

            <section className="workspace-note-summary" aria-labelledby="workspace-note-title">
              <p className="workspace-kicker">Workspace note</p>
              <h4 id="workspace-note-title">Local context</h4>
              <p>Everything here remains in this workspace and can be inspected at the source when you need it.</p>
              <dl aria-label="Workspace totals">
                <div><dt>Sources</dt><dd>{currentOverview.source_count}</dd></div>
                <div><dt>Artifacts</dt><dd>{currentOverview.artifact_count}</dd></div>
                <div><dt>Indexed excerpts</dt><dd>{currentOverview.chunk_count}</dd></div>
                <div><dt>Memory cards</dt><dd>{currentOverview.memory_card_count}</dd></div>
              </dl>
            </section>

            <section className="workspace-continue">
              <div className="workspace-section-heading">
                <div>
                  <p className="workspace-kicker">Next action</p>
                  <h4>Continue working</h4>
                </div>
              </div>
              <p>
                {hasArtifacts
                  ? allArtifactsIndexed
                    ? "Every stored artifact is indexed. Browse the archive or search across retrieved context."
                    : hasIndexedArtifacts
                      ? "Some artifacts are searchable. Open the archive to finish indexing the remaining evidence."
                      : "Artifacts are stored. Open the archive to inspect and index them for retrieval."
                  : "Bring in a repository, folder, file, or pasted note to establish this workspace."}
              </p>
              <div className="workspace-action-row">
                <button className="workspace-primary-action" type="button" onClick={() => onNavigate(hasArtifacts ? "artifacts" : "import")}>
                  {hasArtifacts ? <Archive size={17} /> : <FolderDown size={17} />}
                  {hasArtifacts ? "Open artifacts" : "Import first source"}
                  <ArrowRight size={16} />
                </button>
                {hasArtifacts ? (
                  <button className="workspace-secondary-action" type="button" onClick={() => onNavigate(hasIndexedArtifacts ? "search" : "import")}>
                    {hasIndexedArtifacts ? <Search size={16} /> : <FolderPlus size={16} />}
                    {hasIndexedArtifacts ? "Search workspace" : "Add sources"}
                  </button>
                ) : null}
              </div>
            </section>

            <footer className="workspace-trust-note">
              <Database size={18} aria-hidden="true" />
              <span><strong>Private by default</strong><small>Workspace data is stored in RepoMemo's local application directory.</small></span>
            </footer>
          </>
        ) : (
          <div className="workspace-onboarding-v3">
            <span className="workspace-onboarding-icon"><ShieldLock size={24} /></span>
            <p className="workspace-kicker">Private by design</p>
            <h3>A home for project knowledge</h3>
            <p>Create your first workspace to collect code, documents, decisions, and cited AI output without losing source context.</p>
            <ul>
              <li><CheckCircle2 size={16} /> Stored on this device</li>
              <li><CheckCircle2 size={16} /> Search works without AI</li>
              <li><CheckCircle2 size={16} /> Cloud access stays explicit</li>
            </ul>
          </div>
        )}
      </aside>
    </div>
  );
}

function ImportView({
  artifacts,
  importReport,
  isImporting,
  onImportFiles,
  onImportFolder,
  onPasteImport,
  overview,
  pasteContent,
  pasteLanguage,
  pasteTitle,
  selectedWorkspace,
  setPasteContent,
  setPasteLanguage,
  setPasteTitle,
}: {
  artifacts: ArtifactSummary[];
  importReport: ImportReport | null;
  isImporting: boolean;
  onImportFiles: () => void;
  onImportFolder: () => void;
  onPasteImport: (event: FormEvent<HTMLFormElement>) => void;
  overview: WorkspaceOverview;
  pasteContent: string;
  pasteLanguage: PasteLanguage;
  pasteTitle: string;
  selectedWorkspace: Workspace | undefined;
  setPasteContent: (value: string) => void;
  setPasteLanguage: (value: PasteLanguage) => void;
  setPasteTitle: (value: string) => void;
}) {
  if (!selectedWorkspace) {
    return (
      <section className="panel">
        <EmptyState
          icon={Library}
          title="No workspace selected"
          body="Select or create a workspace before importing artifacts."
        />
      </section>
    );
  }

  return (
    <div className="workbench-grid import-layout graphite-page">
      <section className="panel import-main">
        <PanelHeader icon={FolderDown} label="Import" title="Add local sources" />

        <div className="import-actions">
          <button
            className="import-card"
            type="button"
            onClick={onImportFolder}
            disabled={isImporting}
          >
            <span className="import-card-icon">
              {isImporting ? <Loader2 className="spin" size={22} /> : <FolderDown size={22} />}
            </span>
            <span className="import-card-body">
              <span className="import-card-title">Import folder</span>
              <span className="import-card-sub">Recursively scan a folder for supported files.</span>
            </span>
          </button>
          <button
            className="import-card"
            type="button"
            onClick={onImportFiles}
            disabled={isImporting}
          >
            <span className="import-card-icon">
              <Upload size={22} />
            </span>
            <span className="import-card-body">
              <span className="import-card-title">Import file</span>
              <span className="import-card-sub">Pick individual files from disk.</span>
            </span>
          </button>
        </div>

        <p className="import-accepted">
          <strong>Accepted file types:</strong>{" "}
          <span>
            {ACCEPTED_TEXT_EXTENSIONS.map((ext) => `.${ext}`).join(", ")}
          </span>
          <br />
          <strong>Images:</strong>{" "}
          <span>
            {ACCEPTED_IMAGE_EXTENSIONS.map((ext) => `.${ext}`).join(", ")}
          </span>
        </p>

        <form className="paste-import" onSubmit={onPasteImport}>
          <div className="paste-header">
            <label htmlFor="paste-content">Paste text</label>
            <div className="paste-language">
              <label htmlFor="paste-language">Language</label>
              <select
                id="paste-language"
                value={pasteLanguage}
                onChange={(event) => setPasteLanguage(event.target.value as PasteLanguage)}
                disabled={isImporting}
              >
                {PASTE_LANGUAGES.map((lang) => (
                  <option key={lang} value={lang}>
                    {lang}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <input
            id="paste-title"
            className="paste-title"
            type="text"
            value={pasteTitle}
            onChange={(event) => setPasteTitle(event.target.value)}
            placeholder="Title (optional)"
            disabled={isImporting}
          />
          <textarea
            id="paste-content"
            className="paste-content"
            value={pasteContent}
            onChange={(event) => setPasteContent(event.target.value)}
            placeholder="Paste text, code, or notes here. Stored as a local artifact in this workspace."
            rows={8}
            disabled={isImporting}
          />
          <div className="paste-actions">
            <button
              className="button primary"
              type="submit"
              disabled={isImporting || !pasteContent.trim()}
            >
              {isImporting ? <Loader2 className="spin" size={16} /> : <Upload size={16} />}
              Save pasted text
            </button>
          </div>
        </form>

      </section>

      <aside className="panel import-rail evidence-rail">
        <PanelHeader icon={Database} label="Workspace state" title="Import activity" />
        <MetricGrid overview={overview} />
        <div className="notice">
          Files are copied into this workspace before indexing. You can inspect what was stored before any AI workflow runs.
        </div>
        {importReport ? <ImportReportPanel report={importReport} /> : null}
        <div className="rail-section-heading">
          <div><strong>Recent artifacts</strong><span>Latest local additions</span></div>
          <small>{artifacts.length} stored</small>
        </div>
        <div className="recent-artifact-list">
          {artifacts.length > 0 ? artifacts.slice(0, 6).map((artifact) => (
            <div className="recent-artifact-row" key={artifact.id}>
              <span aria-hidden="true">{artifact.artifact_type === "image" ? <Photo size={15} /> : <FileCode2 size={15} />}</span>
              <div><strong>{artifact.title}</strong><small>{formatArtifactType(artifact.artifact_type)} · {formatRelativeDate(artifact.updated_at)}</small></div>
              <span className={artifact.indexed_at ? "state-dot ready" : "state-dot"} title={artifact.indexed_at ? "Indexed" : "Stored"} />
            </div>
          )) : <p className="rail-empty">Imported files will appear here.</p>}
        </div>
      </aside>
    </div>
  );
}

function ArtifactsView({
  artifactDetail,
  artifacts,
  indexingJob,
  isIndexing,
  onIndexArtifact,
  onIndexWorkspace,
  onRefresh,
  onSelectArtifact,
  overview,
  selectedArtifactId,
  selectedWorkspace,
  symbols,
  summary,
  isSummarizing,
  onSaveMemory,
  onSummarize,
}: {
  artifactDetail: ArtifactDetail | null;
  artifacts: ArtifactSummary[];
  indexingJob: IndexingJobStatus | null;
  isIndexing: boolean;
  onIndexArtifact: (artifactId?: string | null) => void;
  onIndexWorkspace: () => void;
  onRefresh: () => void;
  onSelectArtifact: (id: string) => void;
  overview: WorkspaceOverview;
  selectedArtifactId: string | null;
  selectedWorkspace: Workspace | undefined;
  symbols: Symbol[];
  summary: SummaryResult | null;
  isSummarizing: boolean;
  onSaveMemory: (title: string, body: string, source: string, citations: SummaryResult["citations"]) => void;
  onSummarize: (artifactId: string) => void;
}) {
  const [artifactQuery, setArtifactQuery] = useState("");
  const normalizedArtifactQuery = artifactQuery.trim().toLocaleLowerCase();
  const filteredArtifacts = normalizedArtifactQuery
    ? artifacts.filter((artifact) => [
      artifact.title,
      artifact.path,
      artifact.language ?? "",
      formatArtifactType(artifact.artifact_type),
    ].some((value) => value.toLocaleLowerCase().includes(normalizedArtifactQuery)))
    : artifacts;

  if (!selectedWorkspace) {
    return (
      <section className="panel">
        <EmptyState
          icon={Library}
          title="No workspace selected"
          body="Select or create a workspace before browsing artifacts."
        />
      </section>
    );
  }

  return (
    <div className="artifact-layout graphite-page">
      <section className="panel artifact-main">
        <PanelHeader icon={Archive} label="Artifacts" title="Stored files">
          <button
            className="button primary"
            disabled={isIndexing || artifacts.length === 0}
            type="button"
            onClick={onIndexWorkspace}
          >
            {isIndexing ? <Loader2 className="spin" size={16} /> : <Database size={16} />}
            Index all
          </button>
          <button className="button secondary" type="button" onClick={onRefresh}>
            <RefreshCw size={16} />
            Refresh
          </button>
        </PanelHeader>

        <MetricGrid overview={overview} compact />

        {indexingJob ? <IndexingJobPanel job={indexingJob} /> : null}

        <div className="artifact-library-tools">
          <label className="artifact-search" htmlFor="artifact-search">
            <Search size={17} aria-hidden="true" />
            <span className="sr-only">Search stored artifacts</span>
            <input
              id="artifact-search"
              value={artifactQuery}
              onChange={(event) => setArtifactQuery(event.target.value)}
              placeholder="Search files, paths, or languages"
              type="search"
            />
            {artifactQuery ? <button aria-label="Clear artifact search" className="artifact-search-clear" type="button" onClick={() => setArtifactQuery("")}><X size={15} /></button> : null}
          </label>
          <span className="artifact-result-count" aria-live="polite">{filteredArtifacts.length} of {artifacts.length} files</span>
        </div>

        <div className="artifact-list">
          {artifacts.length === 0 ? (
            <EmptyState
              icon={Archive}
              title="No artifacts indexed yet"
              body="Import a folder, then index it to make its local content searchable and inspectable."
            />
          ) : filteredArtifacts.length === 0 ? (
            <EmptyState
              icon={Search}
              title="No matching artifacts"
              body={`No local files match “${artifactQuery}”. Try a file name, folder, or language.`}
            />
          ) : (
            filteredArtifacts.map((artifact) => (
              <button
                className={
                  artifact.id === selectedArtifactId
                    ? "artifact-row selected"
                    : "artifact-row"
                }
                key={artifact.id}
                type="button"
                onClick={() => onSelectArtifact(artifact.id)}
              >
                <span className="artifact-block-icon" aria-hidden="true">
                  {artifact.artifact_type === "image" ? <Photo size={19} /> : <FileCode2 size={19} />}
                </span>
                <span className="artifact-row-main">
                  <strong>{artifact.title}</strong>
                  <small>{artifact.path}</small>
                </span>
                <span className="artifact-block-footer">
                  <span className="artifact-meta">{artifact.language ?? formatArtifactType(artifact.artifact_type)}</span>
                  <StatusBadge tone={artifact.indexed_at ? "success" : "warning"}>
                    {artifact.indexed_at ? "Indexed" : "Stored"}
                  </StatusBadge>
                </span>
              </button>
            ))
          )}
        </div>
      </section>

      <aside className="panel detail-panel evidence-rail">
        <PanelHeader icon={BookOpen} label="Preview" title="Artifact detail">
          <div className="detail-actions">
            <button
              className="button secondary"
              disabled={!artifactDetail || isSummarizing}
              type="button"
              onClick={() => artifactDetail && onSummarize(artifactDetail.summary.id)}
            >
              {isSummarizing ? <Loader2 className="spin" size={16} /> : <Wand2 size={16} />}
              Summarize
            </button>
            <button
              className="button secondary"
              disabled={!artifactDetail || isIndexing}
              type="button"
              onClick={() => onIndexArtifact(artifactDetail?.summary.id)}
            >
              {isIndexing ? <Loader2 className="spin" size={16} /> : <Database size={16} />}
              {artifactDetail?.summary.artifact_type === "image"
                ? artifactDetail.summary.indexed_at ? "Reanalyze image" : "Analyze image"
                : artifactDetail?.summary.indexed_at ? "Reindex" : "Index"}
            </button>
          </div>
        </PanelHeader>
        {artifactDetail ? (
          <div className="artifact-detail">
            <h3>{artifactDetail.summary.title}</h3>
            <p className="path-text">{artifactDetail.summary.path}</p>
            <div className="detail-meta">
              <StatusBadge tone="neutral">
                {formatArtifactType(artifactDetail.summary.artifact_type)}
              </StatusBadge>
              <StatusBadge tone="neutral">
                {formatBytes(artifactDetail.summary.size_bytes)}
              </StatusBadge>
              <StatusBadge tone="neutral">
                {artifactDetail.summary.source_name}
              </StatusBadge>
              <StatusBadge tone={artifactDetail.summary.indexed_at ? "success" : "warning"}>
                {artifactDetail.summary.indexed_at ? "Ready" : "Stored"}
              </StatusBadge>
              <StatusBadge tone="neutral">
                {artifactDetail.summary.artifact_type === "image"
                  ? `${artifactDetail.chunks.length} visual description${artifactDetail.chunks.length === 1 ? "" : "s"}`
                  : `${artifactDetail.chunks.length} chunks`}
              </StatusBadge>
              <StatusBadge tone="neutral">{symbols.length} symbols</StatusBadge>
            </div>
            {artifactDetail.summary.artifact_type !== "image" ? <SymbolOutline detail={artifactDetail} symbols={symbols} /> : null}
            {summary ? <SummaryPanel summary={summary} onSave={() => onSaveMemory(`Artifact summary: ${artifactDetail.summary.title}`, summary.summary_markdown, "artifact_summary", summary.citations)} /> : null}
            {artifactDetail.summary.artifact_type === "image" ? (
              <p className="image-preview-note">
                The original image stays stored locally. Its searchable representation is the visual description below.
              </p>
            ) : (
              <pre className="content-preview">
                {artifactDetail.content_preview ?? "Preview unavailable."}
              </pre>
            )}
            {artifactDetail.content_truncated ? (
              <p className="truncated-note">Preview truncated to keep the workbench responsive.</p>
            ) : null}
            <ChunkList detail={artifactDetail} />
          </div>
        ) : (
          <EmptyState
            icon={Archive}
            title="No artifact selected"
            body="Select an imported artifact to inspect its stored content."
          />
        )}
      </aside>
    </div>
  );
}

function SearchView({
  artifacts,
  hasSearched,
  isSearching,
  languages,
  onOpenArtifact,
  onOpenSymbol,
  onSearch,
  onSelectResult,
  onSelectSymbol,
  query,
  results,
  selectedResult,
  selectedSymbol,
  setLanguages,
  setQuery,
  setSources,
  setTypes,
  sources,
  symbolResults,
  types,
  workspace,
}: {
  artifacts: ArtifactSummary[];
  hasSearched: boolean;
  isSearching: boolean;
  languages: string[];
  onOpenArtifact: (result: SearchResult) => void;
  onOpenSymbol: (result: SymbolSearchResult) => void;
  onSearch: (event?: FormEvent<HTMLFormElement>) => void;
  onSelectResult: (result: SearchResult) => void;
  onSelectSymbol: (result: SymbolSearchResult) => void;
  query: string;
  results: SearchResult[];
  selectedResult: SearchResult | null;
  selectedSymbol: SymbolSearchResult | null;
  setLanguages: (values: string[]) => void;
  setQuery: (value: string) => void;
  setSources: (values: string[]) => void;
  setTypes: (values: ArtifactSummary["artifact_type"][]) => void;
  sources: string[];
  symbolResults: SymbolSearchResult[];
  types: ArtifactSummary["artifact_type"][];
  workspace: Workspace | undefined;
}) {
  const languageOptions = Array.from(
    new Set(artifacts.flatMap((artifact) => artifact.language ?? [])),
  ).sort();
  const typeOptions = Array.from(
    new Set(artifacts.map((artifact) => artifact.artifact_type)),
  ).sort();
  const sourceOptions = Array.from(
    new Map(artifacts.map((artifact) => [artifact.source_id, artifact.source_name])).entries(),
  ).map(([value, label]) => ({ value, label }));
  const activeFilterCount = languages.length + types.length + sources.length;
  const indexedCount = artifacts.filter((artifact) => artifact.indexed_at).length;
  const coveragePct = artifacts.length === 0 ? 0 : Math.round((indexedCount / artifacts.length) * 100);

  if (!workspace) {
    return (
      <section className="panel">
        <EmptyState icon={Search} title="No workspace selected" body="Select a workspace before searching indexed context." />
      </section>
    );
  }

  return (
    <div className="search-layout graphite-page">
      <section className="panel search-main">
        <PanelHeader icon={Search} label="Local retrieval" title="Search indexed context">
          <StatusBadge tone={artifacts.some((artifact) => artifact.indexed_at) ? "success" : "warning"}>
            {artifacts.filter((artifact) => artifact.indexed_at).length} indexed
          </StatusBadge>
        </PanelHeader>

        <form className="search-form" onSubmit={onSearch}>
          <div className="search-input-wrap">
            <Search size={20} />
            <input
              aria-label="Search workspace"
              autoFocus
              placeholder="Search files, concepts, functions, decisions..."
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            {query ? (
              <button className="clear-search" type="button" aria-label="Clear search" onClick={() => setQuery("")}>
                <X size={16} />
              </button>
            ) : null}
          </div>
          <button className="button primary" disabled={isSearching || !query.trim()} type="submit">
            {isSearching ? <Loader2 className="spin" size={16} /> : <Search size={16} />}
            Search
          </button>
        </form>

        <div className="filter-bar">
          <Filter size={16} />
          <FilterMenu label="Type" options={typeOptions.map((value) => ({ value, label: formatArtifactType(value) }))} selected={types} onChange={(values) => setTypes(values as ArtifactSummary["artifact_type"][])} />
          <FilterMenu label="Language" options={languageOptions.map((value) => ({ value, label: value }))} selected={languages} onChange={setLanguages} />
          <FilterMenu label="Source" options={sourceOptions} selected={sources} onChange={setSources} />
          {activeFilterCount > 0 ? (
            <button className="filter-reset" type="button" onClick={() => { setTypes([]); setLanguages([]); setSources([]); }}>
              Clear {activeFilterCount}
            </button>
          ) : null}
        </div>

        <div className="search-result-summary">
          <span>{hasSearched ? `${symbolResults.length} symbols · ${results.length} contexts` : "Ready to search"}</span>
          <span>{workspace.name}</span>
        </div>

        <div className="search-results">
          {!hasSearched ? (
            <div className="prequery">
              <div className="prequery-hint">
                <Search size={16} />
                <span>Ready to search. Results come from local chunks. No AI, no cloud.</span>
              </div>
              <div className="coverage-strip">
                <span>Index coverage</span>
                <div className="coverage-bar" aria-hidden="true">
                  <i style={{ width: `${coveragePct}%` }} />
                </div>
                <span>{indexedCount}/{artifacts.length} indexed</span>
              </div>
            </div>
          ) : results.length === 0 && symbolResults.length === 0 ? (
            <EmptyState icon={Search} title="No indexed chunks matched" body="Try fewer terms, clear a filter, or index more artifacts." />
          ) : <>
            {symbolResults.length > 0 ? (
              <div className="symbol-results-group">
                <p className="result-group-label">Definitions</p>
                {symbolResults.map((result) => (
                  <button
                    className={selectedSymbol?.symbol.id === result.symbol.id ? "search-result symbol-result selected" : "search-result symbol-result"}
                    key={result.symbol.id}
                    type="button"
                    onClick={() => onSelectSymbol(result)}
                  >
                    <span className="symbol-kind-mark">{symbolKindMark(result.symbol.kind)}</span>
                    <span className="result-content">
                      <span className="result-title-row"><strong>{result.symbol.name}</strong><span>{formatLineRange(result.symbol.start_line, result.symbol.end_line)}</span></span>
                      <code className="symbol-signature">{result.symbol.signature ?? `${result.symbol.kind} ${result.symbol.name}`}</code>
                      <span className="result-meta"><span>{result.path}</span><span>{result.symbol.kind}</span></span>
                    </span>
                  </button>
                ))}
              </div>
            ) : null}
            {results.length > 0 ? <p className="result-group-label">Matched context</p> : null}
            {results.map((result, index) => (
            <button
              className={selectedResult?.chunk_id === result.chunk_id ? "search-result selected" : "search-result"}
              key={result.chunk_id}
              type="button"
              onClick={() => onSelectResult(result)}
            >
              <span className="result-rank">{index + 1}</span>
              <span className="result-content">
                <span className="result-title-row"><strong>{result.title}</strong><span>{formatLineRange(result.start_line, result.end_line)}</span></span>
                <span className="result-path">{result.path}</span>
                <span className="result-snippet">{renderHighlightedSnippet(result.snippet)}</span>
                <span className="result-meta"><span>{result.language ?? formatArtifactType(result.artifact_type)}</span><span>{result.source_name}</span></span>
              </span>
            </button>
            ))}
          </>}
        </div>
      </section>

      <aside className="panel search-preview evidence-rail">
        <PanelHeader icon={BookOpen} label="Matched context" title="Evidence preview">
          <button className="button secondary" disabled={!selectedResult && !selectedSymbol} type="button" onClick={() => selectedSymbol ? onOpenSymbol(selectedSymbol) : selectedResult && onOpenArtifact(selectedResult)}>
            <BookOpen size={16} /> Open artifact
          </button>
        </PanelHeader>
        {selectedSymbol ? (
          <div className="search-evidence symbol-evidence">
            <p className="meta-label">{selectedSymbol.symbol.kind}</p>
            <h3>{selectedSymbol.symbol.name}</h3>
            <p className="path-text">{selectedSymbol.path}</p>
            <div className="detail-meta">
              <StatusBadge tone="neutral">{formatLineRange(selectedSymbol.symbol.start_line, selectedSymbol.symbol.end_line)}</StatusBadge>
              <StatusBadge tone="neutral">{selectedSymbol.language ?? "Code"}</StatusBadge>
              <StatusBadge tone="neutral">{selectedSymbol.source_name}</StatusBadge>
            </div>
            <pre className="symbol-preview">{selectedSymbol.symbol.signature ?? selectedSymbol.symbol.name}</pre>
            <p className="evidence-note">Direct symbol match from the local structural index.</p>
          </div>
        ) : selectedResult ? (
          <div className="search-evidence">
            <h3>{selectedResult.title}</h3>
            <p className="path-text">{selectedResult.path}</p>
            <div className="detail-meta">
              <StatusBadge tone="neutral">{formatLineRange(selectedResult.start_line, selectedResult.end_line)}</StatusBadge>
              <StatusBadge tone="neutral">{selectedResult.language ?? formatArtifactType(selectedResult.artifact_type)}</StatusBadge>
              <StatusBadge tone="neutral">{selectedResult.source_name}</StatusBadge>
            </div>
            <div className="evidence-snippet">{renderHighlightedSnippet(selectedResult.snippet)}</div>
            <p className="evidence-note">This result is retrieved directly from local SQLite FTS. AI is not involved.</p>
          </div>
        ) : (
          <EmptyState icon={BookOpen} title="No result selected" body="Select a search result to inspect its matched source context." />
        )}
      </aside>
    </div>
  );
}

function FilterMenu({ label, onChange, options, selected }: { label: string; onChange: (values: string[]) => void; options: { value: string; label: string }[]; selected: string[] }) {
  return (
    <details className="filter-menu">
      <summary>{label}{selected.length > 0 ? ` ${selected.length}` : ""}</summary>
      <div className="filter-options">
        {options.length === 0 ? <span>No options</span> : options.map((option) => (
          <label key={option.value}>
            <input type="checkbox" checked={selected.includes(option.value)} onChange={() => onChange(selected.includes(option.value) ? selected.filter((value) => value !== option.value) : [...selected, option.value])} />
            <span>{option.label}</span>
          </label>
        ))}
      </div>
    </details>
  );
}

function NavButton({
  active = false,
  disabled = false,
  icon: Icon,
  label,
  onClick,
}: {
  active?: boolean;
  disabled?: boolean;
  icon: typeof Library;
  label: string;
  onClick?: () => void;
}) {
  return (
    <button
      className={active ? "nav-item active" : "nav-item"}
      disabled={disabled}
      type="button"
      onClick={onClick}
      title={disabled ? `${label} is available in a later phase` : label}
    >
      <Icon size={18} />
      {label}
    </button>
  );
}

function PanelHeader({
  children,
  icon: Icon,
  label,
  title,
}: {
  children?: ReactNode;
  icon: typeof Library;
  label: string;
  title: string;
}) {
  return (
    <div className="panel-header">
      <div className="panel-title">
        <div className="panel-icon" aria-hidden="true">
          <Icon size={18} />
        </div>
        <div>
          <p className="meta-label">{label}</p>
          <h3>{title}</h3>
        </div>
      </div>
      {children ? <div className="panel-actions">{children}</div> : null}
    </div>
  );
}

function MetricGrid({
  compact = false,
  overview,
}: {
  compact?: boolean;
  overview: WorkspaceOverview;
}) {
  const metrics = [
    ["Sources", overview.source_count],
    ["Artifacts", overview.artifact_count],
    ["Chunks", overview.chunk_count],
    ["Symbols", overview.symbol_count],
    ["Memory", overview.memory_card_count],
  ];

  return (
    <div className={compact ? "metric-grid compact" : "metric-grid"}>
      {metrics.map(([label, value]) => (
        <div className="metric" key={label}>
          <span>{label}</span>
          <strong>{value}</strong>
        </div>
      ))}
    </div>
  );
}

function ImportReportPanel({ report }: { report: ImportReport }) {
  return (
    <section className="report-panel">
      <div className="report-counts">
        <StatusBadge tone="success">Imported {report.imported}</StatusBadge>
        <StatusBadge tone="neutral">Duplicates {report.duplicates}</StatusBadge>
        <StatusBadge tone={report.skipped > 0 ? "warning" : "neutral"}>
          Skipped {report.skipped}
        </StatusBadge>
        <StatusBadge tone={report.failed > 0 ? "danger" : "neutral"}>
          Failed {report.failed}
        </StatusBadge>
      </div>
      {report.skipped_items.length > 0 ? (
        <details>
          <summary>Skipped and failed items</summary>
          <ul>
            {report.skipped_items.slice(0, 12).map((item) => (
              <li key={`${item.path}-${item.reason}`}>
                <span>{item.reason}</span>
                <code>{item.path}</code>
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </section>
  );
}

function MemoryCardsView({
  body,
  cards,
  detail,
  isSaving,
  onCreate,
  onExport,
  onSearch,
  onSelect,
  searchQuery,
  selectedCardId,
  setBody,
  setTitle,
  title,
  workspace,
}: {
  body: string;
  cards: MemoryCardSummary[];
  detail: MemoryCardDetail | null;
  isSaving: boolean;
  onCreate: () => void;
  onExport: (cardId: string) => void;
  onSearch: (query: string) => void;
  onSelect: (id: string) => void;
  searchQuery: string;
  selectedCardId: string | null;
  setBody: (value: string) => void;
  setTitle: (value: string) => void;
  title: string;
  workspace: Workspace | undefined;
}) {
  if (!workspace) return <EmptyState icon={Bookmark} title="Choose a workspace" body="Memory cards are stored locally in one workspace." />;

  return (
    <div className="memory-layout graphite-page">
      <section className="panel memory-ledger">
        <PanelHeader icon={Bookmark} label="Durable knowledge" title="Memory cards" />
        <label className="memory-search-label" htmlFor="memory-search">Find local memory</label>
        <div className="search-input-wrap memory-search">
          <Search size={17} aria-hidden="true" />
          <input id="memory-search" value={searchQuery} onChange={(event) => onSearch(event.target.value)} placeholder="Search saved knowledge" />
        </div>
        <div className="memory-card-list" aria-label="Saved memory cards">
          {cards.length > 0 ? cards.map((card) => (
            <button className={card.id === selectedCardId ? "memory-card-row selected" : "memory-card-row"} key={card.id} onClick={() => onSelect(card.id)} type="button">
              <span className="memory-row-mark"><Bookmark size={15} /></span>
              <span className="memory-row-main"><strong>{card.title}</strong><small>{card.body_excerpt || "No body text"}</small></span>
              <span className="memory-row-meta"><small>{card.evidence_count} evidence</small><small>{formatRelativeDate(card.updated_at)}</small></span>
            </button>
          )) : <EmptyState icon={Bookmark} title={searchQuery ? "No matching memory" : "No memory saved yet"} body={searchQuery ? "Try a different title or phrase." : "Create a note here, or save a cited briefing or answer."} />}
        </div>
      </section>

      <section className="panel memory-reading-pane">
        <PanelHeader icon={BookOpen} label="Card detail" title={detail?.card.title ?? "Create memory"}>
          {detail ? <button className="button secondary" type="button" onClick={() => onExport(detail.card.id)}><Upload size={16} /> Export Markdown</button> : null}
        </PanelHeader>
        {detail ? <>
          <div className="memory-card-meta"><StatusBadge tone="neutral">{formatMemorySource(detail.card.source)}</StatusBadge><span>Updated {formatRelativeDate(detail.card.updated_at)}</span></div>
          <div className="markdown-content memory-markdown"><ReactMarkdown remarkPlugins={[remarkGfm]}>{detail.card.body_markdown}</ReactMarkdown></div>
          <section className="memory-evidence-list">
            <div className="chunk-panel-header"><strong>Linked evidence</strong><StatusBadge tone={detail.evidence.every((item) => item.exists) ? "success" : "warning"}>{detail.evidence.length} link{detail.evidence.length === 1 ? "" : "s"}</StatusBadge></div>
            {detail.evidence.length > 0 ? detail.evidence.map((evidence) => <div className={evidence.exists ? "memory-evidence-row" : "memory-evidence-row missing"} key={evidence.link_id}><span className={evidence.exists ? "state-dot ready" : "state-dot"} /><div><strong>{evidence.title ?? "Missing evidence"}</strong><code>{evidence.path ?? evidence.target_id}{evidence.start_line ? ` · ${formatLineRange(evidence.start_line, evidence.end_line)}` : ""}</code></div></div>) : <p className="rail-empty">This manual card has no linked source evidence.</p>}
          </section>
        </> : <form className="memory-create-form" onSubmit={(event) => { event.preventDefault(); onCreate(); }}>
          <p className="workspace-summary-intro">Record a durable local note without enabling AI. Add source context in the body when evidence is unavailable.</p>
          <label><span>Title</span><input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Decision, pattern, or operational note" autoFocus /></label>
          <label><span>Memory</span><textarea value={body} onChange={(event) => setBody(event.target.value)} placeholder="Write the knowledge worth keeping…" rows={12} /></label>
          <button className="button primary" disabled={isSaving || !title.trim() || !body.trim()} type="submit">{isSaving ? <Loader2 className="spin" size={16} /> : <Bookmark size={16} />} Save local memory</button>
        </form>}
      </section>

      <aside className="panel evidence-rail memory-rail">
        <PanelHeader icon={ShieldLock} label="Durability" title="Evidence boundary" />
        <div className="trust-callout"><ShieldLock size={18} /><div><strong>Explicit, local, inspectable</strong><p>Nothing is saved automatically. AI-generated cards retain their cited source links; manual cards stay clearly marked.</p></div></div>
        <dl className="settings-facts"><div><dt>Workspace</dt><dd>{workspace.name}</dd></div><div><dt>Saved cards</dt><dd>{cards.length}</dd></div><div><dt>Selected links</dt><dd>{detail?.evidence.length ?? 0}</dd></div></dl>
      </aside>
    </div>
  );
}

function ProviderSettingsView({
  isSaving,
  isTesting,
  onSave,
  onTest,
  provider,
  testResult,
  workspace,
}: {
  isSaving: boolean;
  isTesting: boolean;
  onSave: (settings: ProviderSettings) => void;
  onTest: (providerId: string) => void;
  provider: ProviderSettings | null;
  testResult: ProviderTestResult | null;
  workspace: Workspace | undefined;
}) {
  const [draft, setDraft] = useState<ProviderSettings>(() => provider ?? providerDraft(workspace?.id));

  useEffect(() => {
    setDraft(provider ?? providerDraft(workspace?.id));
  }, [provider, workspace?.id]);

  if (!workspace) {
    return <EmptyState icon={Settings2} title="Choose a workspace" body="Provider settings are saved per workspace." />;
  }

  return (
    <div className="provider-layout graphite-page">
    <section className="provider-workbench panel settings-main">
      <PanelHeader icon={Brain} label="AI provider" title="Provider control">
        <StatusBadge tone={draft.enabled ? "success" : "neutral"}>{draft.enabled ? (draft.provider_type === "openrouter" ? "Cloud" : "Local") : "No AI"}</StatusBadge>
      </PanelHeader>
      <div className="provider-intro">
        <p>RepoMemo only sends content after you explicitly enable a provider. Local Ollama stays on-device; OpenRouter sends selected excerpts to its cloud API.</p>
        <p>Image analysis sends the selected original image to this provider and requires a vision-capable model. Its generated description, not raw pixels, is used for local search.</p>
      </div>
      <form className="provider-form" onSubmit={(event) => { event.preventDefault(); onSave({ ...draft, workspace_id: workspace.id }); }}>
        <label>
          <span>Provider</span>
          <select value={draft.provider_type} onChange={(event) => {
            const cloud = event.target.value === "openrouter";
            setDraft({ ...draft, provider_type: event.target.value, name: cloud ? "OpenRouter" : "Local Ollama", base_url: cloud ? "https://openrouter.ai/api/v1" : "http://127.0.0.1:11434", model: cloud ? "openai/gpt-4o-mini" : "llama3.2", embedding_model: cloud ? null : draft.embedding_model });
          }}>
            <option value="ollama">Ollama-compatible (local)</option>
            <option value="openrouter">OpenRouter (cloud)</option>
          </select>
        </label>
        <label>
          <span>Base URL</span>
          <input value={draft.base_url ?? ""} onChange={(event) => setDraft({ ...draft, base_url: event.target.value })} placeholder="http://127.0.0.1:11434" />
        </label>
        <label>
          <span>Chat model</span>
          <input value={draft.model ?? ""} onChange={(event) => setDraft({ ...draft, model: event.target.value })} placeholder="llama3.2" />
        </label>
        <label>
          <span>Embedding model <em>optional</em></span>
          <input disabled={draft.provider_type === "openrouter"} value={draft.embedding_model ?? ""} onChange={(event) => setDraft({ ...draft, embedding_model: event.target.value || null })} placeholder="nomic-embed-text" />
        </label>
        {draft.provider_type === "openrouter" ? <>
          <label>
            <span>OpenRouter API key</span>
            <input type="password" value={draft.api_key ?? ""} onChange={(event) => setDraft({ ...draft, api_key: event.target.value })} placeholder="sk-or-v1-..." autoComplete="off" />
          </label>
          <label className="provider-toggle cloud-consent">
            <input checked={draft.metadata.cloud_content_acknowledged === true} type="checkbox" onChange={(event) => setDraft({ ...draft, metadata: { ...draft.metadata, cloud_content_acknowledged: event.target.checked } })} />
            <span><strong>I understand content leaves this device</strong><small>RepoMemo sends selected excerpts for AI requests and the original image when you request image analysis.</small></span>
          </label>
        </> : null}
        <label className="provider-toggle">
          <input checked={draft.enabled} type="checkbox" onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })} />
          <span><strong>Enable local AI</strong><small>Only this configured endpoint may receive summary context or images you choose to analyze.</small></span>
        </label>
        <div className="provider-actions">
          <button className="button primary" disabled={isSaving} type="submit">
            {isSaving ? <Loader2 className="spin" size={16} /> : <Settings2 size={16} />} Save provider
          </button>
          <button className="button secondary" disabled={!draft.id || isTesting} type="button" onClick={() => onTest(draft.id)}>
            {isTesting ? <Loader2 className="spin" size={16} /> : <RefreshCw size={16} />} Test connection
          </button>
        </div>
      </form>
      {testResult ? <div className={testResult.success ? "provider-test ready" : "provider-test failed"}>{testResult.success ? <CheckCircle2 size={17} /> : <X size={17} />} {testResult.message}</div> : null}
    </section>
    <aside className="panel evidence-rail settings-rail">
      <PanelHeader icon={ShieldLock} label="Privacy boundary" title="Connection scope" />
      <div className="provider-state-card">
        <span className={draft.enabled ? "state-dot ready" : "state-dot"} />
        <div><strong>{draft.enabled ? `${draft.name} enabled` : "AI disabled"}</strong><p>{draft.provider_type === "openrouter" ? "Selected excerpts and requested images may leave this device." : "Requests are sent only to the configured local endpoint."}</p></div>
      </div>
      <dl className="settings-facts">
        <div><dt>Workspace</dt><dd>{workspace.name}</dd></div>
        <div><dt>Provider type</dt><dd>{draft.provider_type === "openrouter" ? "Cloud" : "Local"}</dd></div>
        <div><dt>Chat model</dt><dd>{draft.model || "Not set"}</dd></div>
        <div><dt>Embeddings</dt><dd>{draft.embedding_model || "Full-text fallback"}</dd></div>
      </dl>
      <div className="trust-callout"><ShieldLock size={18} /><div><strong>Explicit by default</strong><p>Provider access is stored per workspace and remains off until you enable it here.</p></div></div>
    </aside>
    </div>
  );
}

function providerDraft(workspaceId: string | undefined): ProviderSettings {
  return { id: "", workspace_id: workspaceId ?? null, provider_type: "ollama", name: "Local Ollama", base_url: "http://127.0.0.1:11434", model: "llama3.2", embedding_model: null, enabled: false, metadata: {}, api_key: null };
}

function SummaryPanel({ summary, onSave }: { summary: SummaryResult; onSave?: () => void }) {
  return (
    <section className="summary-panel">
      <div className="chunk-panel-header"><strong>Local AI summary</strong><div>{onSave ? <button className="button secondary compact" type="button" onClick={onSave}><Bookmark size={15} /> Save memory</button> : null}<StatusBadge tone="neutral">Cited</StatusBadge></div></div>
      <div className="markdown-content">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{summary.summary_markdown}</ReactMarkdown>
      </div>
      <div className="citation-list">
        {summary.citations.map((citation) => <span key={citation.chunk_id ?? citation.artifact_id}>{formatLineRange(citation.start_line, citation.end_line)}</span>)}
      </div>
      {summary.warnings.map((warning) => <p className="evidence-note" key={warning}>{warning}</p>)}
    </section>
  );
}

function WorkspaceSummaryView({
  isSummarizing,
  onSaveMemory,
  onSummarize,
  summary,
  workspace,
}: {
  isSummarizing: boolean;
  onSaveMemory: (title: string, body: string, source: string, citations: SummaryResult["citations"]) => void;
  onSummarize: () => void;
  summary: SummaryResult | null;
  workspace: Workspace | undefined;
}) {
  if (!workspace) {
    return <EmptyState icon={Wand2} title="Choose a workspace" body="Summaries are generated from that workspace’s indexed artifacts." />;
  }
  return (
    <div className="intelligence-layout graphite-page">
      <section className="workspace-summary panel reading-pane">
        <PanelHeader icon={Wand2} label="Workspace intelligence" title="Project briefing">
          <button className="button primary" disabled={isSummarizing} type="button" onClick={onSummarize}>
            {isSummarizing ? <Loader2 className="spin" size={16} /> : <Wand2 size={16} />} Generate briefing
          </button>
        </PanelHeader>
        <p className="workspace-summary-intro">A concise, cited orientation to <strong>{workspace.name}</strong>, generated only from indexed excerpts in this workspace.</p>
        {summary ? <SummaryPanel summary={summary} onSave={() => onSaveMemory(`Project briefing: ${workspace.name}`, summary.summary_markdown, "workspace_summary", summary.citations)} /> : <EmptyState icon={Brain} title="No briefing generated" body="Index artifacts, configure a provider, then generate a cited project briefing." />}
      </section>
      <aside className="panel evidence-rail summary-rail">
        <PanelHeader icon={ShieldLock} label="Provenance" title="Source grounding" />
        <div className="trust-callout"><ShieldLock size={18} /><div><strong>Evidence stays inspectable</strong><p>The briefing is assembled from local chunks. Citations remain available beside the generated result.</p></div></div>
        <div className="rail-section-heading"><div><strong>Citations</strong><span>Source locations used</span></div><small>{summary?.citations.length ?? 0}</small></div>
        {summary && summary.citations.length > 0 ? (
          <div className="rail-citation-list">{summary.citations.map((citation, index) => <div key={citation.chunk_id ?? citation.artifact_id}><span>{String(index + 1).padStart(2, "0")}</span><code>{formatLineRange(citation.start_line, citation.end_line)}</code></div>)}</div>
        ) : <p className="rail-empty">Generate a briefing to see its source map.</p>}
      </aside>
    </div>
  );
}

function AskView({
  askAnswer,
  isAsking,
  isEmbedding,
  onAsk,
  onEmbed,
  onSaveMemory,
  question,
  setQuestion,
  workspace,
}: {
  askAnswer: AskAnswer | null;
  isAsking: boolean;
  isEmbedding: boolean;
  onAsk: () => void;
  onEmbed: () => void;
  onSaveMemory: (title: string, body: string, source: string, citations: SummaryResult["citations"], confidence?: number | null) => void;
  question: string;
  setQuestion: (value: string) => void;
  workspace: Workspace | undefined;
}) {
  if (!workspace) return <EmptyState icon={Brain} title="Choose a workspace" body="Ask uses the indexed context in one workspace." />;
  return (
    <div className="intelligence-layout ask-layout graphite-page">
    <section className="ask-workbench panel reading-pane">
      <PanelHeader icon={Brain} label="Evidence first" title="Ask the workspace">
        <button className="button secondary" disabled={isEmbedding} type="button" onClick={onEmbed}>{isEmbedding ? <Loader2 className="spin" size={16} /> : <Database size={16} />} Build local embeddings</button>
      </PanelHeader>
      <p className="workspace-summary-intro">Answers are generated only after RepoMemo retrieves inspectable local evidence. Without embeddings, Ask falls back to full-text retrieval.</p>
      <div className="ask-composer">
        <textarea value={question} onChange={(event) => setQuestion(event.target.value)} placeholder={`What do you want to know about ${workspace.name}?`} rows={3} />
        <button className="button primary" disabled={isAsking || !question.trim()} type="button" onClick={onAsk}>{isAsking ? <Loader2 className="spin" size={16} /> : <Brain size={16} />} Ask</button>
      </div>
      {askAnswer ? <div className="ask-answer">
        <div className="chunk-panel-header"><strong>Answer</strong><div><button className="button secondary compact" type="button" onClick={() => onSaveMemory(`Answer: ${question.trim().slice(0, 72)}`, askAnswer.answer_markdown, "answer", askAnswer.citations, askAnswer.confidence)}><Bookmark size={15} /> Save memory</button><StatusBadge tone={askAnswer.confidence && askAnswer.confidence > 0.15 ? "success" : "warning"}>{askAnswer.confidence && askAnswer.confidence > 0.15 ? "Evidence found" : "Low confidence"}</StatusBadge></div></div>
        <div className="markdown-content"><ReactMarkdown remarkPlugins={[remarkGfm]}>{askAnswer.answer_markdown}</ReactMarkdown></div>
        <div className="citation-list">{askAnswer.citations.map((citation) => <span key={citation.chunk_id ?? citation.artifact_id}>{citation.title} · {formatLineRange(citation.start_line, citation.end_line)}</span>)}</div>
        {askAnswer.warnings.map((warning) => <p className="evidence-note" key={warning}>{warning}</p>)}
        <div className="retrieved-evidence answer-evidence"><p className="result-group-label">Retrieved context</p>{askAnswer.retrieved_context.map((result) => <div key={result.chunk_id}><strong>{result.title}</strong><span>{formatLineRange(result.start_line, result.end_line)}</span><p>{result.snippet}</p></div>)}</div>
      </div> : <EmptyState icon={Brain} title="Ask with evidence" body="Enter a question to retrieve local context before generation." />}
    </section>
    <aside className="panel evidence-rail ask-evidence-rail">
      <PanelHeader icon={BookOpen} label="Retrieved context" title="Evidence rail" />
      {askAnswer ? <>
        <div className="rail-section-heading"><div><strong>Sources used</strong><span>Inspectable local matches</span></div><small>{askAnswer.retrieved_context.length}</small></div>
        <div className="retrieved-evidence">{askAnswer.retrieved_context.map((result) => <div key={result.chunk_id}><div><strong>{result.title}</strong><span>{formatLineRange(result.start_line, result.end_line)}</span></div><p>{result.snippet}</p></div>)}</div>
        <div className="rail-section-heading"><div><strong>Citations</strong><span>Linked answer references</span></div><small>{askAnswer.citations.length}</small></div>
        <div className="citation-list">{askAnswer.citations.map((citation) => <span key={citation.chunk_id ?? citation.artifact_id}>{citation.title} · {formatLineRange(citation.start_line, citation.end_line)}</span>)}</div>
      </> : <EmptyState icon={BookOpen} title="No evidence retrieved" body="Ask a question to populate the evidence rail." />}
    </aside>
    </div>
  );
}

function IndexingJobPanel({ job }: { job: IndexingJobStatus }) {
  const total = job.progress_total ?? job.progress_current;
  const progressText =
    total > 0 ? `${job.progress_current}/${total}` : `${job.progress_current}`;
  const normalizedStatus = job.status.toLowerCase();
  const failed = normalizedStatus === "failed" || normalizedStatus === "error";
  const completed = normalizedStatus === "completed";
  const active = ["running", "indexing", "processing", "active"].includes(normalizedStatus);
  const statusTone = failed ? "danger" : completed ? "success" : active ? "warning" : "neutral";
  const statusLabel = failed ? "Failed" : completed ? "Ready" : active ? "Indexing" : "Queued";

  return (
    <section className="report-panel">
      <div className="report-counts">
        <StatusBadge tone={statusTone}>
          {statusLabel}
        </StatusBadge>
        <StatusBadge tone="neutral">{job.stage.replace(/_/g, " ")}</StatusBadge>
        <StatusBadge tone="neutral">{progressText}</StatusBadge>
      </div>
      {job.error_message ? <p className="job-error">{job.error_message}</p> : null}
    </section>
  );
}

function SymbolOutline({ detail, symbols }: { detail: ArtifactDetail; symbols: Symbol[] }) {
  const supported = ["TypeScript", "JavaScript", "Python", "Rust"].includes(
    detail.summary.language ?? "",
  );

  return (
    <section className="symbol-panel">
      <div className="chunk-panel-header">
        <strong>File outline</strong>
        <StatusBadge tone={symbols.length > 0 ? "success" : "neutral"}>
          {symbols.length} definitions
        </StatusBadge>
      </div>
      {symbols.length > 0 ? (
        <div className="symbol-list">
          {symbols.map((symbol) => (
            <div className="symbol-row" key={symbol.id} tabIndex={0}>
              <span className="symbol-kind-mark">{symbolKindMark(symbol.kind)}</span>
              <span className="symbol-row-main">
                <strong>{symbol.name}</strong>
                <code>{symbol.signature ?? symbol.kind}</code>
              </span>
              <span className="symbol-line">{formatLineRange(symbol.start_line, symbol.end_line)}</span>
            </div>
          ))}
        </div>
      ) : (
        <p className="chunk-empty">
          {supported
            ? "No definitions were found in this file."
            : "No symbol index for this language yet."}
        </p>
      )}
    </section>
  );
}

function ChunkList({ detail }: { detail: ArtifactDetail }) {
  const isImage = detail.summary.artifact_type === "image";
  const label = isImage ? "Visual description" : "Chunks";
  if (detail.chunks.length === 0) {
    return (
      <section className="chunk-panel">
        <div className="chunk-panel-header">
          <strong>{label}</strong>
          <StatusBadge tone="warning">{isImage ? "Needs vision model" : "Not indexed"}</StatusBadge>
        </div>
        <p className="chunk-empty">
          {isImage
            ? "Enable a vision-capable provider, then choose Analyze image. RepoMemo will extract visible text, code, and visual structure into one searchable description."
            : "Run Index to generate line-based chunks for search and citations."}
        </p>
      </section>
    );
  }

  return (
    <section className="chunk-panel">
      <div className="chunk-panel-header">
        <strong>{label}</strong>
        <StatusBadge tone="success">{detail.chunks.length} ready</StatusBadge>
      </div>
      <div className="chunk-list">
        {detail.chunks.map((chunk) => (
          <article className="chunk-row" key={chunk.id || chunk.chunk_index}>
            <div className="chunk-row-header">
              <strong>{isImage ? "Generated description" : `Chunk ${chunk.chunk_index + 1}`}</strong>
              {!isImage ? <span>{formatLineRange(chunk.start_line, chunk.end_line)}</span> : null}
            </div>
            {chunk.heading_path ? (
              <p className="chunk-heading">{chunk.heading_path}</p>
            ) : null}
            <p className="chunk-preview">{chunk.text}</p>
          </article>
        ))}
      </div>
    </section>
  );
}

function symbolKindMark(kind: Symbol["kind"]): string {
  const marks: Record<Symbol["kind"], string> = {
    function: "ƒ",
    class: "C",
    method: "m",
    interface: "I",
    enum: "E",
    route: "R",
    endpoint: "↗",
    config: "⚙",
    test: "T",
  };
  return marks[kind];
}

function StatusBadge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "success" | "warning" | "danger";
}) {
  return <span className={`status-badge ${tone}`}>{children}</span>;
}

function EmptyState({
  body,
  icon: Icon,
  title,
}: {
  body: string;
  icon: typeof Library;
  title: string;
}) {
  return (
    <div className="empty-state">
      <Icon size={24} />
      <strong>{title}</strong>
      <p>{body}</p>
    </div>
  );
}

function formatArtifactType(value: string) {
  return value.replace(/_/g, " ");
}

function formatMemorySource(source: string) {
  const labels: Record<string, string> = {
    manual: "Manual note",
    artifact_summary: "Artifact summary",
    workspace_summary: "Project briefing",
    answer: "Cited answer",
  };
  return labels[source] ?? source.replace(/_/g, " ");
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatLineRange(start: number | null, end: number | null) {
  if (start && end) {
    return start === end ? `line ${start}` : `lines ${start}-${end}`;
  }

  return "lines unknown";
}

function renderHighlightedSnippet(snippet: string): ReactNode {
  return snippet.split(/(<mark>|<\/mark>)/).map((part, index, parts) => {
    if (part === "<mark>" || part === "</mark>") return null;
    const highlighted = parts.slice(0, index).filter((item) => item === "<mark>").length >
      parts.slice(0, index).filter((item) => item === "</mark>").length;
    return highlighted ? <mark key={`${index}-${part}`}>{part}</mark> : part;
  });
}

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}

function formatRelativeDate(value: string) {
  const timestamp = new Date(value).getTime();
  const elapsed = Date.now() - timestamp;

  if (!Number.isFinite(timestamp) || elapsed < 0) {
    return formatDate(value);
  }

  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;

  return new Date(value).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}
