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
} from "@tabler/icons-react";
import {
  chooseImportFiles,
  chooseImportFolder,
  createWorkspace,
  getAppSettings,
  getArtifact,
  getWorkspaceOverview,
  indexArtifact,
  indexWorkspace,
  importPaths,
  listArtifacts,
  listWorkspaces,
} from "./lib/repomemoApi";
import type {
  AppSettings,
  ArtifactDetail,
  ArtifactSummary,
  IndexingJobStatus,
  ImportReport,
  Workspace,
  WorkspaceOverview,
} from "./types";

type LoadState = "idle" | "loading" | "ready" | "error";
type View = "workspaces" | "import" | "artifacts";
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
  const [manualPath, setManualPath] = useState("");
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

  async function handleManualImport(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const paths = manualPath
      .split(/\r?\n/)
      .map((path) => path.trim())
      .filter(Boolean);
    await runImport(paths);
    setManualPath("");
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
          <NavButton disabled icon={Search} label="Search" />
          <NavButton disabled icon={Brain} label="Ask" />
          <NavButton disabled icon={Sparkles} label="Memory cards" />
          <NavButton disabled icon={Settings2} label="Settings" />
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
            <p className="meta-label">Phase 1B</p>
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
            manualPath={manualPath}
            onImportFiles={handleImportFiles}
            onImportFolder={handleImportFolder}
            onManualImport={handleManualImport}
            overview={currentOverview}
            selectedWorkspace={selectedWorkspace}
            setManualPath={setManualPath}
          />
        ) : (
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
  manualPath,
  onImportFiles,
  onImportFolder,
  onManualImport,
  overview,
  selectedWorkspace,
  setManualPath,
}: {
  artifacts: ArtifactSummary[];
  importReport: ImportReport | null;
  isImporting: boolean;
  manualPath: string;
  onImportFiles: () => void;
  onImportFolder: () => void;
  onManualImport: (event: FormEvent<HTMLFormElement>) => void;
  overview: WorkspaceOverview;
  selectedWorkspace: Workspace | undefined;
  setManualPath: (value: string) => void;
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

        <div className="import-toolbar">
          <button
            className="button primary"
            type="button"
            onClick={onImportFolder}
            disabled={isImporting}
          >
            {isImporting ? <Loader2 className="spin" size={16} /> : <FolderDown size={16} />}
            Import folder
          </button>
          <button
            className="button secondary"
            type="button"
            onClick={onImportFiles}
            disabled={isImporting}
          >
            <Upload size={16} />
            Import files
          </button>
        </div>

        <form className="manual-import" onSubmit={onManualImport}>
          <label htmlFor="manual-path">Manual path import</label>
          <div className="manual-row">
            <textarea
              id="manual-path"
              value={manualPath}
              onChange={(event) => setManualPath(event.target.value)}
              placeholder="Paste one file or folder path per line"
            />
            <button className="button secondary" type="submit" disabled={isImporting}>
              Import
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

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}
