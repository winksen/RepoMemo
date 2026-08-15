/*
THESIS: Shared mode is a clear trust boundary, not a local-preview replica; identity precedes workspace access.
OWN-WORLD: The existing evidence instrument stays light and precise, with white surfaces, quiet seams, and reserved green-blue action.
STORY: Sign in, establish an organization, and create a server-authoritative workspace without mistaking local data for shared data.
FIRST VIEWPORT: A calm explanatory field balances a focused access form; after authentication, an organization rail frames the workspace ledger.
FORM: The established RepoMemo operate composition, adapted into a protected browser session with direct API-state feedback.
*/

import type { FormEvent } from "react";
import { useEffect, useMemo, useState } from "react";
import {
  IconArrowRight as ArrowRight,
  IconArrowLeft as ArrowLeft,
  IconBuildingCommunity as Building,
  IconChevronRight as ChevronRight,
  IconFileText as FileText,
  IconKey as Key,
  IconLoader2 as Loader,
  IconLogout as Logout,
  IconRefresh as Refresh,
  IconSearch as Search,
  IconPlus as Plus,
  IconShieldLock as Shield,
  IconStack2 as Layers,
  IconUsers as Users,
} from "@tabler/icons-react";
import {
  createSharedOrganization,
  createSharedMemoryCard,
  createSharedTextArtifact,
  createSharedWorkspace,
  getSharedSession,
  getSharedWorkspaceOverview,
  indexSharedWorkspace,
  listSharedArtifacts,
  listSharedMemoryCards,
  listSharedOrganizations,
  listSharedWorkspaces,
  loginSharedUser,
  registerSharedUser,
  searchSharedWorkspace,
  sharedApiUrl,
  type SharedApiError,
} from "./lib/sharedApi";
import type {
  ArtifactSummary,
  MemoryCardSummary,
  Organization,
  SearchResult,
  SharedSession,
  SharedWorkspace,
  WorkspaceOverview,
} from "./types";

const SESSION_STORAGE_KEY = "repomemo.shared.access-token";

type AuthMode = "sign-in" | "sign-up";
type PageState = "restoring" | "unauthenticated" | "ready" | "error";

export function SharedWebApp() {
  const [pageState, setPageState] = useState<PageState>("restoring");
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [session, setSession] = useState<SharedSession | null>(null);
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [workspaces, setWorkspaces] = useState<SharedWorkspace[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const storedToken = window.sessionStorage.getItem(SESSION_STORAGE_KEY);
    if (!storedToken) {
      setPageState("unauthenticated");
      return;
    }
    hydrate(storedToken).catch(() => {
      window.sessionStorage.removeItem(SESSION_STORAGE_KEY);
      setPageState("unauthenticated");
    });
  }, []);

  async function hydrate(token: string) {
    setPageState("restoring");
    setError(null);
    try {
      const [nextSession, nextOrganizations, nextWorkspaces] = await Promise.all([
        getSharedSession(token),
        listSharedOrganizations(token),
        listSharedWorkspaces(token),
      ]);
      setAccessToken(token);
      setSession(nextSession);
      setOrganizations(nextOrganizations);
      setWorkspaces(nextWorkspaces);
      setPageState("ready");
    } catch (requestError) {
      setError(apiMessage(requestError));
      setPageState("error");
      throw requestError;
    }
  }

  function handleAuthenticated(token: string) {
    window.sessionStorage.setItem(SESSION_STORAGE_KEY, token);
    hydrate(token).catch(() => undefined);
  }

  function signOut() {
    window.sessionStorage.removeItem(SESSION_STORAGE_KEY);
    setAccessToken(null);
    setSession(null);
    setOrganizations([]);
    setWorkspaces([]);
    setError(null);
    setPageState("unauthenticated");
  }

  if (pageState === "restoring") {
    return <LoadingCanvas label="Restoring your shared session" />;
  }

  if (pageState === "unauthenticated" || pageState === "error") {
    return (
      <AuthCanvas
        initialError={pageState === "error" ? error : null}
        onAuthenticated={handleAuthenticated}
      />
    );
  }

  if (!accessToken || !session) {
    return <LoadingCanvas label="Preparing shared workspace" />;
  }

  return (
    <SharedWorkspaceHome
      accessToken={accessToken}
      organizations={organizations}
      session={session}
      setError={setError}
      setOrganizations={setOrganizations}
      setWorkspaces={setWorkspaces}
      signOut={signOut}
      workspaces={workspaces}
    />
  );
}

function LoadingCanvas({ label }: { label: string }) {
  return (
    <main className="shared-auth-canvas">
      <div className="shared-loading" role="status"><Loader className="spin" size={22} /> {label}</div>
    </main>
  );
}

function AuthCanvas({
  initialError,
  onAuthenticated,
}: {
  initialError: string | null;
  onAuthenticated: (token: string) => void;
}) {
  const [mode, setMode] = useState<AuthMode>("sign-in");
  const [displayName, setDisplayName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(initialError);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true);
    setError(null);
    try {
      const response = mode === "sign-up"
        ? await registerSharedUser({ email, displayName, password })
        : await loginSharedUser({ email, password });
      onAuthenticated(response.access_token);
    } catch (requestError) {
      setError(apiMessage(requestError));
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <main className="shared-auth-canvas">
      <section className="shared-auth-intro" aria-label="RepoMemo shared mode">
        <div className="shared-brand"><span className="shared-brand-glyph"><Layers size={21} /></span><strong>RepoMemo</strong></div>
        <p className="shared-eyebrow">Shared workspace</p>
        <h1>One evidence base for the team.</h1>
        <p className="shared-intro-copy">Sign in to the server-authoritative workspace. Your browser only receives data allowed by your signed session.</p>
        <dl className="shared-trust-list">
          <div><dt><Shield size={16} /> Server boundary</dt><dd>Shared data is requested through the protected API.</dd></div>
          <div><dt><Key size={16} /> Session token</dt><dd>Access expires and is kept only for this browser session.</dd></div>
          <div><dt><Users size={16} /> Membership-aware</dt><dd>Workspace access is scoped to the signed-in account.</dd></div>
        </dl>
      </section>

      <section className="shared-auth-form-region" aria-label="Account access">
        <div className="shared-auth-form-shell">
          <div className="shared-auth-switch" role="tablist" aria-label="Account action">
            <button className={mode === "sign-in" ? "active" : ""} onClick={() => { setMode("sign-in"); setError(null); }} role="tab" type="button">Sign in</button>
            <button className={mode === "sign-up" ? "active" : ""} onClick={() => { setMode("sign-up"); setError(null); }} role="tab" type="button">Create account</button>
          </div>
          <div className="shared-auth-heading">
            <p className="shared-eyebrow">{mode === "sign-in" ? "Welcome back" : "Start a shared workspace"}</p>
            <h2>{mode === "sign-in" ? "Continue to RepoMemo" : "Create your account"}</h2>
            <p>{mode === "sign-in" ? "Use the credentials registered with this server." : "Your account will be the initial owner of any organization you create."}</p>
          </div>
          <form className="shared-auth-form" onSubmit={submit}>
            {mode === "sign-up" ? <label>Display name<input autoComplete="name" onChange={(event) => setDisplayName(event.target.value)} placeholder="Ada Lovelace" required value={displayName} /></label> : null}
            <label>Email<input autoComplete="email" onChange={(event) => setEmail(event.target.value)} placeholder="you@company.com" required type="email" value={email} /></label>
            <label>Password<input autoComplete={mode === "sign-up" ? "new-password" : "current-password"} minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={password} /></label>
            {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
            <button className="shared-primary-action" disabled={isSubmitting} type="submit">
              {isSubmitting ? <Loader className="spin" size={17} /> : <ArrowRight size={17} />}
              {mode === "sign-in" ? "Sign in to shared mode" : "Create account and continue"}
            </button>
          </form>
          <p className="shared-api-note">Connected to <code>{sharedApiUrl}</code></p>
        </div>
      </section>
    </main>
  );
}

function SharedWorkspaceHome({
  accessToken,
  organizations,
  session,
  setError,
  setOrganizations,
  setWorkspaces,
  signOut,
  workspaces,
}: {
  accessToken: string;
  organizations: Organization[];
  session: SharedSession;
  setError: (value: string | null) => void;
  setOrganizations: (value: Organization[] | ((current: Organization[]) => Organization[])) => void;
  setWorkspaces: (value: SharedWorkspace[] | ((current: SharedWorkspace[]) => SharedWorkspace[])) => void;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  const [organizationName, setOrganizationName] = useState("");
  const [workspaceName, setWorkspaceName] = useState("");
  const [organizationId, setOrganizationId] = useState(organizations[0]?.id ?? "");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [openedWorkspace, setOpenedWorkspace] = useState<SharedWorkspace | null>(null);

  useEffect(() => {
    if (!organizationId && organizations[0]) setOrganizationId(organizations[0].id);
  }, [organizationId, organizations]);

  const currentOrganization = useMemo(
    () => organizations.find((organization) => organization.id === organizationId),
    [organizationId, organizations],
  );

  async function createOrganization(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setFormError(null); setError(null);
    try {
      const organization = await createSharedOrganization(accessToken, organizationName);
      setOrganizations((current) => [...current, organization]);
      setOrganizationId(organization.id);
      setOrganizationName("");
    } catch (requestError) { setFormError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function createWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!organizationId) return;
    setIsSubmitting(true); setFormError(null); setError(null);
    try {
      const workspace = await createSharedWorkspace(accessToken, organizationId, workspaceName);
      setWorkspaces((current) => [workspace, ...current]);
      setWorkspaceName("");
    } catch (requestError) { setFormError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  if (openedWorkspace) {
    return <SharedWorkspaceDetail accessToken={accessToken} onBack={() => setOpenedWorkspace(null)} workspace={openedWorkspace} />;
  }

  return (
    <main className="shared-home">
      <header className="shared-home-header">
        <div className="shared-brand"><span className="shared-brand-glyph"><Layers size={19} /></span><strong>RepoMemo</strong><span className="shared-mode-tag">Shared</span></div>
        <div className="shared-user-menu"><span>{session.user.display_name}</span><button onClick={signOut} type="button"><Logout size={16} /> Sign out</button></div>
      </header>
      <div className="shared-home-frame">
        <aside className="shared-home-rail">
          <p className="shared-eyebrow">Organization</p>
          {organizations.length ? <div className="shared-organization-list">{organizations.map((organization) => <button className={organization.id === organizationId ? "selected" : ""} key={organization.id} onClick={() => setOrganizationId(organization.id)} type="button"><Building size={16} /><span>{organization.name}</span></button>)}</div> : <p className="shared-rail-empty">Create an organization to establish your team boundary.</p>}
          <div className="shared-rail-footer"><Shield size={15} /><span>JWT session active</span></div>
        </aside>
        <section className="shared-home-content">
          <div className="shared-page-heading"><p className="shared-eyebrow">Shared workspaces</p><h1>{currentOrganization ? currentOrganization.name : "Set up your team"}</h1><p>{currentOrganization ? "Workspaces are server-authoritative and available only to their members." : "First create the organization that will own your team’s shared memory."}</p></div>
          {organizations.length === 0 ? <form className="shared-setup-form" onSubmit={createOrganization}><label>Organization name<input autoFocus onChange={(event) => setOrganizationName(event.target.value)} placeholder="Engineering" required value={organizationName} /></label><button className="shared-primary-action" disabled={isSubmitting} type="submit">{isSubmitting ? <Loader className="spin" size={17} /> : <Plus size={17} />} Create organization</button></form> : <>
            <form className="shared-create-workspace" onSubmit={createWorkspace}><div><strong>Create a workspace</strong><span>Start a bounded memory space for a repository, system, or initiative.</span></div><label className="sr-only" htmlFor="shared-workspace-name">Workspace name</label><input id="shared-workspace-name" onChange={(event) => setWorkspaceName(event.target.value)} placeholder="Payments platform" required value={workspaceName} /><button className="shared-primary-action" disabled={isSubmitting} type="submit">{isSubmitting ? <Loader className="spin" size={17} /> : <Plus size={17} />} Create workspace</button></form>
            <div className="shared-workspace-list">{workspaces.filter((workspace) => workspace.organization_id === organizationId).map(({ workspace, role }) => <article className="shared-workspace-row" key={workspace.id}><span className="shared-workspace-icon"><Layers size={18} /></span><div><h2>{workspace.name}</h2><p>Created {new Date(workspace.created_at).toLocaleDateString()} · Your role: {role}</p></div><button aria-label={`Open ${workspace.name}`} onClick={() => setOpenedWorkspace(workspaces.find((entry) => entry.workspace.id === workspace.id) ?? null)} type="button"><ChevronRight size={18} /></button></article>)}{workspaces.filter((workspace) => workspace.organization_id === organizationId).length === 0 ? <div className="shared-empty-state"><Layers size={26} /><strong>No workspaces yet</strong><span>Create the first shared workspace for {currentOrganization?.name}.</span></div> : null}</div>
          </>}
          {formError ? <p className="shared-form-error" role="alert">{formError}</p> : null}
          <p className="shared-boundary-note"><Shield size={15} /> Every workspace view is loaded through the JWT-protected API. This browser never falls back to local preview data.</p>
        </section>
      </div>
    </main>
  );
}

function SharedWorkspaceDetail({
  accessToken,
  onBack,
  workspace,
}: {
  accessToken: string;
  onBack: () => void;
  workspace: SharedWorkspace;
}) {
  const [overview, setOverview] = useState<WorkspaceOverview | null>(null);
  const [artifacts, setArtifacts] = useState<ArtifactSummary[]>([]);
  const [memoryCards, setMemoryCards] = useState<MemoryCardSummary[]>([]);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [query, setQuery] = useState("");
  const [noteTitle, setNoteTitle] = useState("");
  const [noteContent, setNoteContent] = useState("");
  const [memoryTitle, setMemoryTitle] = useState("");
  const [memoryBody, setMemoryBody] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const canWrite = workspace.role !== "viewer";

  async function load() {
    setIsLoading(true);
    setError(null);
    try {
      const [nextOverview, nextArtifacts, nextMemory] = await Promise.all([
        getSharedWorkspaceOverview(accessToken, workspace.workspace.id),
        listSharedArtifacts(accessToken, workspace.workspace.id),
        listSharedMemoryCards(accessToken, workspace.workspace.id),
      ]);
      setOverview(nextOverview);
      setArtifacts(nextArtifacts);
      setMemoryCards(nextMemory);
    } catch (requestError) {
      setError(apiMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }

  useEffect(() => { void load(); }, [accessToken, workspace.workspace.id]);

  async function addNote(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      await createSharedTextArtifact(accessToken, workspace.workspace.id, {
        title: noteTitle,
        content: noteContent,
        language: "Markdown",
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
      });
      setMemoryTitle(""); setMemoryBody("");
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function runSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!query.trim()) return;
    setIsSubmitting(true); setError(null);
    try { setResults(await searchSharedWorkspace(accessToken, workspace.workspace.id, query)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function runIndex() {
    setIsSubmitting(true); setError(null);
    try { await indexSharedWorkspace(accessToken, workspace.workspace.id); await load(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
    <main className="shared-home">
      <header className="shared-home-header">
        <div className="shared-brand"><span className="shared-brand-glyph"><Layers size={19} /></span><strong>RepoMemo</strong><span className="shared-mode-tag">Shared</span></div>
        <button className="shared-back-button" onClick={onBack} type="button"><ArrowLeft size={16} /> All workspaces</button>
      </header>
      <section className="shared-detail-shell" aria-busy={isLoading}>
        <div className="shared-detail-heading">
          <div><p className="shared-eyebrow">Server workspace · {workspace.role}</p><h1>{workspace.workspace.name}</h1><p>Artifacts, search results, and durable team memory are all retrieved through the protected shared API.</p></div>
          <div className="shared-detail-actions"><button className="shared-secondary-action" disabled={isLoading} onClick={() => void load()} type="button"><Refresh size={16} /> Refresh</button>{canWrite ? <button className="shared-primary-action" disabled={isSubmitting || isLoading} onClick={() => void runIndex()} type="button">{isSubmitting ? <Loader className="spin" size={16} /> : <Layers size={16} />} Index workspace</button> : null}</div>
        </div>
        {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
        <div className="shared-evidence-summary">
          <span><strong>{overview?.artifact_count ?? "—"}</strong> artifacts</span><span><strong>{overview?.chunk_count ?? "—"}</strong> indexed chunks</span><span><strong>{overview?.memory_card_count ?? "—"}</strong> memory cards</span>
        </div>
        <div className="shared-detail-grid">
          <section className="shared-detail-panel">
            <div className="shared-panel-heading"><div><FileText size={18} /><h2>Evidence ledger</h2></div><span>{artifacts.length} records</span></div>
            {artifacts.length ? <div className="shared-artifact-list">{artifacts.map((artifact) => <article key={artifact.id}><div><strong>{artifact.title}</strong><span>{artifact.path} · {artifact.indexed_at ? "Indexed" : "Not indexed"}</span></div><small>{artifact.language ?? artifact.artifact_type}</small></article>)}</div> : <div className="shared-empty-state"><FileText size={25} /><strong>No shared evidence yet</strong><span>Add a pasted note below, then index it when you are ready to search.</span></div>}
            {canWrite ? <form className="shared-note-form" onSubmit={addNote}><h3>Add shared note</h3><input onChange={(event) => setNoteTitle(event.target.value)} placeholder="Decision or implementation note" required value={noteTitle} /><textarea onChange={(event) => setNoteContent(event.target.value)} placeholder="Paste Markdown, code context, or a meeting note…" required value={noteContent} /><button className="shared-primary-action" disabled={isSubmitting} type="submit"><Plus size={16} /> Store evidence</button></form> : <p className="shared-readonly-note"><Shield size={15} /> Your viewer membership can inspect shared evidence but cannot change it.</p>}
          </section>
          <aside className="shared-detail-panel shared-retrieval-panel">
            <div className="shared-panel-heading"><div><Search size={18} /><h2>Retrieve</h2></div></div>
            <form className="shared-search-form" onSubmit={runSearch}><input onChange={(event) => setQuery(event.target.value)} placeholder="Search indexed evidence" value={query} /><button className="shared-secondary-action" disabled={isSubmitting} type="submit">Search</button></form>
            {results.length ? <div className="shared-search-results">{results.map((result) => <article key={result.chunk_id}><strong>{result.title}</strong><p>{result.snippet}</p><span>{result.path}{result.start_line ? ` · line ${result.start_line}` : ""}</span></article>)}</div> : <p className="shared-muted-copy">Index one or more artifacts, then search the evidence base from here.</p>}
            <div className="shared-memory-section"><div className="shared-panel-heading"><div><Shield size={18} /><h2>Team memory</h2></div><span>{memoryCards.length}</span></div>{memoryCards.length ? <div className="shared-memory-list">{memoryCards.map((card) => <article key={card.id}><strong>{card.title}</strong><span>{card.body_excerpt}</span></article>)}</div> : <p className="shared-muted-copy">No durable memory cards yet.</p>}{canWrite ? <form className="shared-memory-form" onSubmit={addMemory}><input onChange={(event) => setMemoryTitle(event.target.value)} placeholder="Memory title" required value={memoryTitle} /><textarea onChange={(event) => setMemoryBody(event.target.value)} placeholder="A concise durable fact…" required value={memoryBody} /><button className="shared-secondary-action" disabled={isSubmitting} type="submit">Save memory</button></form> : null}</div>
          </aside>
        </div>
      </section>
    </main>
  );
}

function apiMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) return String((error as SharedApiError).message);
  return "The shared API could not be reached. Confirm that the server is running and try again.";
}
