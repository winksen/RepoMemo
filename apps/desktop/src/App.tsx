import type { FormEvent, ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import {
  IconArchive as Archive,
  IconBook as BookOpen,
  IconBrain as Brain,
  IconCircleCheck as CheckCircle2,
  IconDatabase as Database,
  IconFileCode as FileCode2,
  IconFolderDown as FolderDown,
  IconFolderPlus as FolderPlus,
  IconFilter as Filter,
  IconDeviceSdCard as HardDrive,
  IconStack2 as Layers3,
  IconLibrary as Library,
  IconLoader2 as Loader2,
  IconMoon as Moon,
  IconRefresh as RefreshCw,
  IconSearch as Search,
  IconSettings as Settings2,
  IconSparkles as Sparkles,
  IconSun as Sun,
  IconUpload as Upload,
  IconX as X,
} from "@tabler/icons-react";
import {
  ACCEPTED_IMAGE_EXTENSIONS,
  ACCEPTED_TEXT_EXTENSIONS,
  chooseImportFiles,
  chooseImportFolder,
  createWorkspace,
  askWorkspace,
  embedWorkspace,
  getAppSettings,
  getArtifact,
  getWorkspaceOverview,
  indexArtifact,
  indexWorkspace,
  importPaths,
  importText,
  listArtifacts,
  listProviderSettings,
  listSymbols,
  listWorkspaces,
  PASTE_LANGUAGES,
  searchWorkspace,
  searchSymbols,
  saveProviderSettings,
  summarizeArtifact,
  summarizeWorkspace,
  testProvider,
  type PasteLanguage,
} from "./lib/repomemoApi";
import type {
  AppSettings,
  AskAnswer,
  ArtifactDetail,
  ArtifactSummary,
  IndexingJobStatus,
  ImportReport,
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
type View = "workspaces" | "import" | "artifacts" | "search" | "summary" | "ask" | "settings";
type ThemeMode = "light" | "dark";

const emptyOverview = (workspaceId: string): WorkspaceOverview => ({
  workspace_id: workspaceId,
  source_count: 0,
  artifact_count: 0,
  chunk_count: 0,
  symbol_count: 0,
  memory_card_count: 0,
});

export function App() {
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
  const [theme, setTheme] = useState<ThemeMode>(() => {
    const saved = localStorage.getItem("repomemo-theme");
    if (saved === "light" || saved === "dark") {
      return saved;
    }
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  });

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("repomemo-theme", theme);
  }, [theme]);

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
      return;
    }

    refreshWorkspaceData(selectedWorkspaceId);
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

  const selectedWorkspace = useMemo(
    () => workspaces.find((workspace) => workspace.id === selectedWorkspaceId),
    [selectedWorkspaceId, workspaces],
  );

  const currentOverview = overview ?? emptyOverview(selectedWorkspaceId ?? "");

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
              icon={Sparkles}
              label="Summary"
              onClick={() => setActiveView("summary")}
            />
            <NavButton active={activeView === "ask"} disabled={!selectedWorkspace} icon={Brain} label="Ask" onClick={() => setActiveView("ask")} />
            <NavButton disabled icon={Sparkles} label="Memory cards" />
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
        <header className="workbench-header">
          <div>
            <p className="meta-label">Phase 1F</p>
            <h2>{selectedWorkspace?.name ?? "Workspace foundation"}</h2>
          </div>
          <div className="header-actions">
            <StatusBadge tone={loadState === "error" ? "danger" : "success"}>
              <CheckCircle2 size={15} />
              {loadState === "loading" ? "Booting core" : "Local core ready"}
            </StatusBadge>
            <StatusBadge tone={settings?.ai_enabled ? "success" : "neutral"}>
              {settings?.ai_enabled ? `${providers.find((item) => item.enabled)?.provider_type === "openrouter" ? "Cloud" : "Local"} · ${settings.active_provider}` : "No AI"}
            </StatusBadge>
            <StatusBadge tone={providers.find((item) => item.enabled)?.provider_type === "openrouter" ? "success" : "neutral"}>
              {providers.find((item) => item.enabled)?.provider_type === "openrouter" ? "Cloud enabled" : "Cloud off"}
            </StatusBadge>
            <button
              aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} mode`}
              className="icon-button"
              type="button"
              onClick={() => setTheme((current) => (current === "dark" ? "light" : "dark"))}
              title={`Switch to ${theme === "dark" ? "light" : "dark"} mode`}
            >
              {theme === "dark" ? <Sun size={17} /> : <Moon size={17} />}
            </button>
          </div>
        </header>

        {errorMessage ? <div className="error-banner">{errorMessage}</div> : null}

        {activeView === "workspaces" ? (
          <WorkspaceView
            currentOverview={currentOverview}
            onCreateWorkspace={handleCreateWorkspace}
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
            summary={workspaceSummary}
            workspace={selectedWorkspace}
          />
        ) : activeView === "ask" ? (
          <AskView askAnswer={askAnswer} isAsking={isAsking} isEmbedding={isEmbedding} onAsk={handleAsk} onEmbed={handleEmbedWorkspace} question={askQuestion} setQuestion={setAskQuestion} workspace={selectedWorkspace} />
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
    </main>
  );
}

function WorkspaceView({
  currentOverview,
  onCreateWorkspace,
  onSelectWorkspace,
  selectedWorkspaceId,
  setWorkspaceName,
  workspaceName,
  workspaces,
}: {
  currentOverview: WorkspaceOverview;
  onCreateWorkspace: (event: FormEvent<HTMLFormElement>) => void;
  onSelectWorkspace: (id: string) => void;
  selectedWorkspaceId: string | null;
  setWorkspaceName: (value: string) => void;
  workspaceName: string;
  workspaces: Workspace[];
}) {
  return (
    <div className="workbench-grid">
      <section className="panel">
        <PanelHeader
          icon={FolderPlus}
          label="Workspace"
          title="Create or select"
        />
        <form className="workspace-form" onSubmit={onCreateWorkspace}>
          <label htmlFor="workspace-name">Workspace name</label>
          <div className="input-row">
            <input
              id="workspace-name"
              value={workspaceName}
              onChange={(event) => setWorkspaceName(event.target.value)}
              placeholder="Engineering memory"
            />
            <button className="button primary" type="submit">
              Create
            </button>
          </div>
        </form>

        <div className="workspace-list" aria-live="polite">
          {workspaces.length === 0 ? (
            <EmptyState
              icon={Library}
              title="No workspaces yet"
              body="Create a local workspace to start importing technical memory."
            />
          ) : (
            workspaces.map((workspace) => (
              <button
                className={
                  workspace.id === selectedWorkspaceId
                    ? "workspace-row selected"
                    : "workspace-row"
                }
                key={workspace.id}
                type="button"
                onClick={() => onSelectWorkspace(workspace.id)}
              >
                <span>
                  <strong>{workspace.name}</strong>
                  <small>{formatDate(workspace.updated_at)}</small>
                </span>
                <span className="row-dot" aria-hidden="true" />
              </button>
            ))
          )}
        </div>
      </section>

      <section className="panel">
        <PanelHeader
          icon={Database}
          label="Selected workspace"
          title="Local memory state"
        />
        <MetricGrid overview={currentOverview} />
        <div className="notice">
          Phase 1A is running. Phase 1B adds import and artifact storage before
          indexing, search, AI, or memory cards.
        </div>
      </section>
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
    <div className="workbench-grid">
      <section className="panel">
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

        <MetricGrid overview={overview} compact />

        {importReport ? <ImportReportPanel report={importReport} /> : null}
      </section>

      <section className="panel">
        <PanelHeader icon={Database} label="Workspace" title="Import readiness" />
        <MetricGrid overview={overview} />
        <div className="notice">
          Stored artifacts are browseable now. Chunking and full-text search are
          next, so imported content stays local and inspectable before AI enters
          the workflow.
        </div>
        <div className="recent-strip">
          <strong>Recent artifacts</strong>
          <span>{artifacts.length} stored</span>
        </div>
      </section>
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
  onSummarize: (artifactId: string) => void;
}) {
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
    <div className="artifact-layout">
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

        <div className="artifact-list">
          {artifacts.length === 0 ? (
            <EmptyState
              icon={Archive}
              title="No artifacts indexed yet"
              body="Import a folder to make this workspace inspectable. Search starts after chunking in Phase 1C/1D."
            />
          ) : (
            artifacts.map((artifact) => (
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
                <FileCode2 size={18} />
                <span className="artifact-row-main">
                  <strong>{artifact.title}</strong>
                  <small>{artifact.path}</small>
                </span>
                <span className="artifact-meta">
                  {artifact.language ?? formatArtifactType(artifact.artifact_type)}
                </span>
                <StatusBadge tone={artifact.indexed_at ? "success" : "warning"}>
                  {artifact.indexed_at ? "Indexed" : "Stored"}
                </StatusBadge>
              </button>
            ))
          )}
        </div>
      </section>

      <aside className="panel detail-panel">
        <PanelHeader icon={BookOpen} label="Preview" title="Artifact detail">
          <div className="detail-actions">
            <button
              className="button secondary"
              disabled={!artifactDetail || isSummarizing}
              type="button"
              onClick={() => artifactDetail && onSummarize(artifactDetail.summary.id)}
            >
              {isSummarizing ? <Loader2 className="spin" size={16} /> : <Sparkles size={16} />}
              Summarize
            </button>
            <button
              className="button secondary"
              disabled={!artifactDetail || isIndexing}
              type="button"
              onClick={() => onIndexArtifact(artifactDetail?.summary.id)}
            >
              {isIndexing ? <Loader2 className="spin" size={16} /> : <Database size={16} />}
              {artifactDetail?.summary.indexed_at ? "Reindex" : "Index"}
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
                {artifactDetail.chunks.length} chunks
              </StatusBadge>
              <StatusBadge tone="neutral">{symbols.length} symbols</StatusBadge>
            </div>
            <SymbolOutline detail={artifactDetail} symbols={symbols} />
            {summary ? <SummaryPanel summary={summary} /> : null}
            <pre className="content-preview">
              {artifactDetail.content_preview ?? "Preview unavailable."}
            </pre>
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
    <div className="search-layout">
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
                <span>Ready to search. Results come from local chunks — no AI, no cloud.</span>
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

      <aside className="panel search-preview">
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
    <section className="provider-workbench panel">
      <PanelHeader icon={Brain} label="AI provider" title="Provider control">
        <StatusBadge tone={draft.enabled ? "success" : "neutral"}>{draft.enabled ? (draft.provider_type === "openrouter" ? "Cloud" : "Local") : "No AI"}</StatusBadge>
      </PanelHeader>
      <div className="provider-intro">
        <p>RepoMemo only sends content after you explicitly enable a provider. Local Ollama stays on-device; OpenRouter sends selected excerpts to its cloud API.</p>
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
            <span><strong>I understand excerpts leave this device</strong><small>RepoMemo sends only the cited excerpts needed for a requested summary to OpenRouter.</small></span>
          </label>
        </> : null}
        <label className="provider-toggle">
          <input checked={draft.enabled} type="checkbox" onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })} />
          <span><strong>Enable local AI</strong><small>Only this configured endpoint may receive summary context.</small></span>
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
  );
}

function providerDraft(workspaceId: string | undefined): ProviderSettings {
  return { id: "", workspace_id: workspaceId ?? null, provider_type: "ollama", name: "Local Ollama", base_url: "http://127.0.0.1:11434", model: "llama3.2", embedding_model: null, enabled: false, metadata: {}, api_key: null };
}

function SummaryPanel({ summary }: { summary: SummaryResult }) {
  return (
    <section className="summary-panel">
      <div className="chunk-panel-header"><strong>Local AI summary</strong><StatusBadge tone="neutral">Cited</StatusBadge></div>
      <p className="summary-body">{summary.summary_markdown}</p>
      <div className="citation-list">
        {summary.citations.map((citation) => <span key={citation.chunk_id ?? citation.artifact_id}>{formatLineRange(citation.start_line, citation.end_line)}</span>)}
      </div>
      {summary.warnings.map((warning) => <p className="evidence-note" key={warning}>{warning}</p>)}
    </section>
  );
}

function WorkspaceSummaryView({
  isSummarizing,
  onSummarize,
  summary,
  workspace,
}: {
  isSummarizing: boolean;
  onSummarize: () => void;
  summary: SummaryResult | null;
  workspace: Workspace | undefined;
}) {
  if (!workspace) {
    return <EmptyState icon={Sparkles} title="Choose a workspace" body="Summaries are generated from that workspace’s indexed artifacts." />;
  }
  return (
    <section className="workspace-summary panel">
      <PanelHeader icon={Sparkles} label="Workspace intelligence" title="Project briefing">
        <button className="button primary" disabled={isSummarizing} type="button" onClick={onSummarize}>
          {isSummarizing ? <Loader2 className="spin" size={16} /> : <Sparkles size={16} />} Summarize workspace
        </button>
      </PanelHeader>
      <p className="workspace-summary-intro">Create a concise briefing from indexed excerpts across <strong>{workspace.name}</strong>. Every generated result includes the local chunks it used.</p>
      {summary ? <SummaryPanel summary={summary} /> : <EmptyState icon={Brain} title="No workspace summary yet" body="Index artifacts, configure a provider, then generate a cited project briefing." />}
    </section>
  );
}

function AskView({
  askAnswer,
  isAsking,
  isEmbedding,
  onAsk,
  onEmbed,
  question,
  setQuestion,
  workspace,
}: {
  askAnswer: AskAnswer | null;
  isAsking: boolean;
  isEmbedding: boolean;
  onAsk: () => void;
  onEmbed: () => void;
  question: string;
  setQuestion: (value: string) => void;
  workspace: Workspace | undefined;
}) {
  if (!workspace) return <EmptyState icon={Brain} title="Choose a workspace" body="Ask uses the indexed context in one workspace." />;
  return (
    <section className="ask-workbench panel">
      <PanelHeader icon={Brain} label="Evidence first" title="Ask the workspace">
        <button className="button secondary" disabled={isEmbedding} type="button" onClick={onEmbed}>{isEmbedding ? <Loader2 className="spin" size={16} /> : <Database size={16} />} Build local embeddings</button>
      </PanelHeader>
      <p className="workspace-summary-intro">Answers are generated only after RepoMemo retrieves inspectable local evidence. Without embeddings, Ask falls back to full-text retrieval.</p>
      <div className="ask-composer">
        <textarea value={question} onChange={(event) => setQuestion(event.target.value)} placeholder={`What do you want to know about ${workspace.name}?`} rows={3} />
        <button className="button primary" disabled={isAsking || !question.trim()} type="button" onClick={onAsk}>{isAsking ? <Loader2 className="spin" size={16} /> : <Brain size={16} />} Ask</button>
      </div>
      {askAnswer ? <div className="ask-answer">
        <div className="chunk-panel-header"><strong>Answer</strong><StatusBadge tone={askAnswer.confidence && askAnswer.confidence > 0.15 ? "success" : "warning"}>{askAnswer.confidence && askAnswer.confidence > 0.15 ? "Evidence found" : "Low confidence"}</StatusBadge></div>
        <p className="summary-body">{askAnswer.answer_markdown}</p>
        <div className="citation-list">{askAnswer.citations.map((citation) => <span key={citation.chunk_id ?? citation.artifact_id}>{citation.title} · {formatLineRange(citation.start_line, citation.end_line)}</span>)}</div>
        {askAnswer.warnings.map((warning) => <p className="evidence-note" key={warning}>{warning}</p>)}
        <div className="retrieved-evidence"><p className="result-group-label">Retrieved context</p>{askAnswer.retrieved_context.map((result) => <div key={result.chunk_id}><strong>{result.title}</strong><span>{formatLineRange(result.start_line, result.end_line)}</span><p>{result.snippet}</p></div>)}</div>
      </div> : <EmptyState icon={Brain} title="Ask with evidence" body="Enter a question to retrieve local context before generation." />}
    </section>
  );
}

function IndexingJobPanel({ job }: { job: IndexingJobStatus }) {
  const total = job.progress_total ?? job.progress_current;
  const progressText =
    total > 0 ? `${job.progress_current}/${total}` : `${job.progress_current}`;

  return (
    <section className="report-panel">
      <div className="report-counts">
        <StatusBadge tone={job.status === "completed" ? "success" : "warning"}>
          {job.status === "completed" ? "Ready" : "Indexing"}
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
  if (detail.chunks.length === 0) {
    return (
      <section className="chunk-panel">
        <div className="chunk-panel-header">
          <strong>Chunks</strong>
          <StatusBadge tone="warning">Not indexed</StatusBadge>
        </div>
        <p className="chunk-empty">
          Run Index to generate line-based chunks for search and citations.
        </p>
      </section>
    );
  }

  return (
    <section className="chunk-panel">
      <div className="chunk-panel-header">
        <strong>Chunks</strong>
        <StatusBadge tone="success">{detail.chunks.length} ready</StatusBadge>
      </div>
      <div className="chunk-list">
        {detail.chunks.map((chunk) => (
          <article className="chunk-row" key={chunk.id || chunk.chunk_index}>
            <div className="chunk-row-header">
              <strong>Chunk {chunk.chunk_index + 1}</strong>
              <span>{formatLineRange(chunk.start_line, chunk.end_line)}</span>
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
