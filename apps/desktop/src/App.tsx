import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  Archive,
  Brain,
  CheckCircle2,
  Database,
  FolderPlus,
  HardDrive,
  Layers3,
  Library,
  Search,
  Settings2,
  Sparkles,
} from "lucide-react";
import {
  createWorkspace,
  getAppSettings,
  listWorkspaces,
} from "./lib/repomemoApi";
import type { AppSettings, Workspace } from "./types";

type LoadState = "idle" | "loading" | "ready" | "error";

const phaseCards = [
  {
    title: "Workspace Core",
    status: "Active",
    icon: Library,
    detail: "Create and reopen local memory workspaces backed by SQLite.",
  },
  {
    title: "Local Storage",
    status: "Ready",
    icon: Database,
    detail: "Metadata schema, app data folder, and blob-store folders are prepared.",
  },
  {
    title: "Search Loop",
    status: "Next",
    icon: Search,
    detail: "Import, chunk, and FTS retrieval begin after the skeleton is stable.",
  },
];

export function App() {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(
    null,
  );
  const [workspaceName, setWorkspaceName] = useState("");
  const [loadState, setLoadState] = useState<LoadState>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

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

  const selectedWorkspace = useMemo(
    () => workspaces.find((workspace) => workspace.id === selectedWorkspaceId),
    [selectedWorkspaceId, workspaces],
  );

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
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand-mark">
          <div className="brand-glyph">
            <Layers3 size={24} strokeWidth={1.8} />
          </div>
          <div>
            <p className="eyebrow">Local memory</p>
            <h1>RepoMemo</h1>
          </div>
        </div>

        <nav className="nav-list" aria-label="Primary">
          <button className="nav-item active" type="button">
            <Library size={18} />
            Workspaces
          </button>
          <button className="nav-item" type="button" disabled>
            <Archive size={18} />
            Artifacts
          </button>
          <button className="nav-item" type="button" disabled>
            <Search size={18} />
            Search
          </button>
          <button className="nav-item" type="button" disabled>
            <Brain size={18} />
            Ask
          </button>
          <button className="nav-item" type="button" disabled>
            <Settings2 size={18} />
            Settings
          </button>
        </nav>

        <section className="storage-panel" aria-label="Storage settings">
          <div className="panel-icon">
            <HardDrive size={18} />
          </div>
          <div>
            <p className="panel-label">Storage root</p>
            <p className="storage-path">{settings?.data_dir ?? "Loading..."}</p>
          </div>
        </section>
      </aside>

      <section className="workspace-area">
        <header className="workspace-header">
          <div>
            <p className="eyebrow">Phase 1A</p>
            <h2>Workspace foundation</h2>
          </div>
          <div className="status-pill">
            <CheckCircle2 size={16} />
            {loadState === "loading" ? "Booting core" : "Core bridge ready"}
          </div>
        </header>

        {errorMessage ? <div className="error-banner">{errorMessage}</div> : null}

        <div className="content-grid">
          <section className="workspace-panel">
            <div className="section-heading">
              <div>
                <p className="eyebrow">Create</p>
                <h3>Local workspace</h3>
              </div>
              <FolderPlus size={21} />
            </div>

            <form className="workspace-form" onSubmit={handleCreateWorkspace}>
              <label htmlFor="workspace-name">Workspace name</label>
              <div className="input-row">
                <input
                  id="workspace-name"
                  value={workspaceName}
                  onChange={(event) => setWorkspaceName(event.target.value)}
                  placeholder="Engineering memory"
                />
                <button type="submit">Create</button>
              </div>
            </form>

            <div className="workspace-list" aria-live="polite">
              {workspaces.length === 0 ? (
                <div className="empty-state">
                  <Library size={28} />
                  <p>Create the first RepoMemo workspace to initialize local metadata.</p>
                </div>
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
                    onClick={() => setSelectedWorkspaceId(workspace.id)}
                  >
                    <span>
                      <strong>{workspace.name}</strong>
                      <small>{new Date(workspace.updated_at).toLocaleString()}</small>
                    </span>
                    <span className="row-dot" />
                  </button>
                ))
              )}
            </div>
          </section>

          <section className="workspace-detail">
            <div className="section-heading">
              <div>
                <p className="eyebrow">Selected</p>
                <h3>{selectedWorkspace?.name ?? "No workspace yet"}</h3>
              </div>
              <Sparkles size={21} />
            </div>

            <div className="detail-stack">
              {phaseCards.map((card) => {
                const Icon = card.icon;
                return (
                  <article className="phase-card" key={card.title}>
                    <div className="phase-icon">
                      <Icon size={20} />
                    </div>
                    <div>
                      <div className="phase-title-row">
                        <h4>{card.title}</h4>
                        <span>{card.status}</span>
                      </div>
                      <p>{card.detail}</p>
                    </div>
                  </article>
                );
              })}
            </div>
          </section>
        </div>
      </section>
    </main>
  );
}
