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
  getAppSettings,
  getArtifact,
  getWorkspaceOverview,
  indexArtifact,
  indexWorkspace,
  importPaths,
  importText,
  listArtifacts,
  listWorkspaces,
  PASTE_LANGUAGES,
  searchWorkspace,
  type PasteLanguage,
} from "./lib/repomemoApi";
import type {
  AppSettings,
  ArtifactDetail,
  ArtifactSummary,
  IndexingJobStatus,
  ImportReport,
  SearchResult,
  Workspace,
  WorkspaceOverview,
} from "./types";

type LoadState = "idle" | "loading" | "ready" | "error";
type View = "workspaces" | "import" | "artifacts" | "search";
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
      return;
    }

    refreshWorkspaceData(selectedWorkspaceId);
  }, [selectedWorkspaceId]);

  useEffect(() => {
    let cancelled = false;

    async function loadArtifact() {
      if (!selectedArtifactId) {
        setArtifactDetail(null);
        return;
      }

      try {
        const detail = await getArtifact(selectedArtifactId);
        if (!cancelled) {
          setArtifactDetail(detail);
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
      setSelectedSearchResult(null);
      setHasSearched(false);
      return;
    }

    setIsSearching(true);
    setErrorMessage(null);
    try {
      const results = await searchWorkspace({
        workspace_id: selectedWorkspaceId,
        query: searchQuery.trim(),
        artifact_types: searchTypes,
        languages: searchLanguages,
        source_ids: searchSources,
        limit: 40,
      });
      setSearchResults(results);
      setSelectedSearchResult(results[0] ?? null);
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
            <NavButton disabled icon={Brain} label="Ask" />
            <NavButton disabled icon={Sparkles} label="Memory cards" />
          </div>
          <div className="nav-divider" aria-hidden="true" />
          <div className="nav-group">
            <NavButton disabled icon={Settings2} label="Settings" />
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
            <p className="meta-label">Phase 1D</p>
            <h2>{selectedWorkspace?.name ?? "Workspace foundation"}</h2>
          </div>
          <div className="header-actions">
            <StatusBadge tone={loadState === "error" ? "danger" : "success"}>
              <CheckCircle2 size={15} />
              {loadState === "loading" ? "Booting core" : "Local core ready"}
            </StatusBadge>
            <StatusBadge tone="neutral">Cloud off</StatusBadge>
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
          />
        ) : (
          <SearchView
            artifacts={artifacts}
            hasSearched={hasSearched}
            isSearching={isSearching}
            languages={searchLanguages}
            onOpenArtifact={openSearchArtifact}
            onSearch={handleSearch}
            onSelectResult={setSelectedSearchResult}
            query={searchQuery}
            results={searchResults}
            selectedResult={selectedSearchResult}
            setLanguages={setSearchLanguages}
            setQuery={setSearchQuery}
            setSources={setSearchSources}
            setTypes={setSearchTypes}
            sources={searchSources}
            types={searchTypes}
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
          <button
            className="button secondary"
            disabled={!artifactDetail || isIndexing}
            type="button"
            onClick={() => onIndexArtifact(artifactDetail?.summary.id)}
          >
            {isIndexing ? <Loader2 className="spin" size={16} /> : <Database size={16} />}
            {artifactDetail?.summary.indexed_at ? "Reindex" : "Index"}
          </button>
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
            </div>
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
  onSearch,
  onSelectResult,
  query,
  results,
  selectedResult,
  setLanguages,
  setQuery,
  setSources,
  setTypes,
  sources,
  types,
  workspace,
}: {
  artifacts: ArtifactSummary[];
  hasSearched: boolean;
  isSearching: boolean;
  languages: string[];
  onOpenArtifact: (result: SearchResult) => void;
  onSearch: (event?: FormEvent<HTMLFormElement>) => void;
  onSelectResult: (result: SearchResult) => void;
  query: string;
  results: SearchResult[];
  selectedResult: SearchResult | null;
  setLanguages: (values: string[]) => void;
  setQuery: (value: string) => void;
  setSources: (values: string[]) => void;
  setTypes: (values: ArtifactSummary["artifact_type"][]) => void;
  sources: string[];
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
          <span>{hasSearched ? `${results.length} results` : "Ready to search"}</span>
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
          ) : results.length === 0 ? (
            <EmptyState icon={Search} title="No indexed chunks matched" body="Try fewer terms, clear a filter, or index more artifacts." />
          ) : results.map((result, index) => (
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
        </div>
      </section>

      <aside className="panel search-preview">
        <PanelHeader icon={BookOpen} label="Matched context" title="Evidence preview">
          <button className="button secondary" disabled={!selectedResult} type="button" onClick={() => selectedResult && onOpenArtifact(selectedResult)}>
            <BookOpen size={16} /> Open artifact
          </button>
        </PanelHeader>
        {selectedResult ? (
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
