/*
THESIS: Shared mode is a clear trust boundary, not a local-preview replica; identity precedes workspace access.
OWN-WORLD: The existing evidence instrument stays light and precise, with white surfaces, quiet seams, and reserved green-blue action.
STORY: Sign in, establish an organization, and create a server-authoritative workspace without mistaking local data for shared data.
FIRST VIEWPORT: A calm explanatory field balances a focused access form; after authentication, an organization rail frames the workspace ledger.
FORM: The established RepoMemo operate composition, adapted into a protected browser session with direct API-state feedback.
*/

import type { FormEvent, ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import {
  IconArrowRight as ArrowRight,
  IconArrowLeft as ArrowLeft,
  IconBuildingCommunity as Building,
  IconBrain as Brain,
  IconChevronRight as ChevronRight,
  IconFileText as FileText,
  IconLayoutDashboard as Dashboard,
  IconBook2 as Book,
  IconKey as Key,
  IconLoader2 as Loader,
  IconLogout as Logout,
  IconRefresh as Refresh,
  IconSearch as Search,
  IconPlus as Plus,
  IconPencil as Pencil,
  IconShieldLock as Shield,
  IconStack2 as Layers,
  IconTimeline as Timeline,
  IconTrash as Trash,
  IconUsers as Users,
  IconUpload as Upload,
} from "@tabler/icons-react";
import {
  createSharedOrganization,
  createSharedMemoryCard,
  createSharedTextArtifact,
  createSharedWorkspace,
  deleteSharedArtifact,
  deleteSharedMemoryCard,
  deleteSharedWorkspace,
  exportSharedMemoryCard,
  getSharedArtifact,
  getSharedHealth,
  getSharedWorkspaceCapabilities,
  getSharedMemoryCard,
  getSharedSession,
  getSharedWorkspaceOverview,
  indexSharedWorkspace,
  indexSharedArtifact,
  listSharedArtifacts,
  listSharedWorkspaceAiProviders,
  listSharedWorkspaceActivity,
  listSharedWorkspaceMembers,
  listSharedMemoryCards,
  listSharedOrganizations,
  listSharedWorkspaces,
  loginSharedUser,
  generateSharedWorkspaceAiOverview,
  registerSharedUser,
  searchSharedWorkspace,
  saveSharedWorkspaceAiProvider,
  removeSharedWorkspaceMember,
  sharedApiUrl,
  upsertSharedWorkspaceMember,
  uploadSharedArtifact,
  updateSharedArtifact,
  updateSharedMemoryCard,
  updateSharedWorkspace,
  type SharedApiError,
} from "./lib/sharedApi";
import type {
  ArtifactSummary,
  ArtifactDetail,
  MemoryCardDetail,
  MemoryCardSummary,
  Organization,
  SearchResult,
  SharedAiProviderSettings,
  SharedSession,
  SharedWorkspace,
  WorkspaceMember,
  WorkspaceActivityEvent,
  WorkspaceAiOverview,
  WorkspaceCapabilities,
  WorkspaceRole,
  WorkspaceOverview,
} from "./types";
import { Button } from "./components/ui/button";
import { Dropdown } from "./components/ui/dropdown";
import { Input } from "./components/ui/input";
import { Textarea } from "./components/ui/textarea";

const SESSION_STORAGE_KEY = "repomemo.shared.access-token";

type AuthMode = "sign-in" | "sign-up";
type PageState = "restoring" | "unauthenticated" | "ready" | "error";
type WorkspaceSection = "overview" | "evidence" | "retrieval" | "memory" | "people" | "activity";

const WORKSPACE_SECTIONS: WorkspaceSection[] = ["overview", "evidence", "retrieval", "memory", "people", "activity"];

function currentPathname() {
  return window.location.pathname.replace(/\/+$/, "") || "/";
}

function navigate(to: string, replace = false) {
  if (currentPathname() === to) return;
  window.history[replace ? "replaceState" : "pushState"]({}, "", to);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function SharedWebApp() {
  const [pageState, setPageState] = useState<PageState>("restoring");
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [session, setSession] = useState<SharedSession | null>(null);
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [workspaces, setWorkspaces] = useState<SharedWorkspace[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pathname, setPathname] = useState(currentPathname);
  const [apiAvailable, setApiAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    const syncPathname = () => setPathname(currentPathname());
    window.addEventListener("popstate", syncPathname);
    return () => window.removeEventListener("popstate", syncPathname);
  }, []);

  useEffect(() => {
    getSharedHealth().then(() => setApiAvailable(true)).catch(() => setApiAvailable(false));
  }, []);

  useEffect(() => {
    const storedToken = window.sessionStorage.getItem(SESSION_STORAGE_KEY);
    if (!storedToken) {
      if (currentPathname() === "/" || currentPathname().startsWith("/workspaces")) {
        navigate("/login", true);
      }
      setPageState("unauthenticated");
      return;
    }
    hydrate(storedToken).catch(() => {
      window.sessionStorage.removeItem(SESSION_STORAGE_KEY);
      navigate("/login", true);
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
    navigate("/workspaces", true);
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
    navigate("/login", true);
  }

  useEffect(() => {
    if (pageState === "ready" && (pathname === "/" || pathname === "/login" || pathname === "/register")) {
      navigate("/workspaces", true);
    }
  }, [pageState, pathname]);

  if (pageState === "restoring") {
    return <LoadingCanvas label="Restoring your shared session" />;
  }

  if (pageState === "unauthenticated" || pageState === "error") {
    return (
      <AuthCanvas
        initialMode={pathname === "/register" ? "sign-up" : "sign-in"}
        initialError={pageState === "error" ? error : null}
        apiAvailable={apiAvailable}
        onAuthenticated={handleAuthenticated}
      />
    );
  }

  if (!accessToken || !session) {
    return <LoadingCanvas label="Preparing shared workspace" />;
  }

  const routeParts = pathname.split("/").filter(Boolean).map((part) => decodeURIComponent(part));
  const workspaceId = routeParts[0] === "workspaces" ? routeParts[1] ?? null : null;
  const workspaceSection = routeParts[2] as WorkspaceSection | undefined;
  const activeWorkspaceSection = workspaceSection && WORKSPACE_SECTIONS.includes(workspaceSection)
    ? workspaceSection
    : undefined;
  const workspace = workspaceId
    ? workspaces.find((entry) => entry.workspace.id === workspaceId)
    : undefined;
  const workspaceOrganization = workspace
    ? organizations.find((organization) => organization.id === workspace.organization_id)
    : undefined;

  if (workspace && routeParts[2] === "artifacts" && routeParts[3]) {
    return <SharedArtifactDetail accessToken={accessToken} apiAvailable={apiAvailable} artifactId={routeParts[3]} onBack={() => navigate(`/workspaces/${encodeURIComponent(workspaceId!)}/overview`)} organization={workspaceOrganization} session={session} signOut={signOut} workspace={workspace} />;
  }

  if (workspace && routeParts[2] === "memory-cards" && routeParts[3]) {
    return <SharedMemoryCardDetail accessToken={accessToken} apiAvailable={apiAvailable} cardId={routeParts[3]} onBack={() => navigate(`/workspaces/${encodeURIComponent(workspaceId!)}/overview`)} organization={workspaceOrganization} session={session} signOut={signOut} workspace={workspace} />;
  }

  const isWorkspaceSectionRoute = routeParts.length === 2 || (Boolean(activeWorkspaceSection) && routeParts.length === 3);

  if (workspaceId && !isWorkspaceSectionRoute) {
    return <SharedRouteNotFound apiAvailable={apiAvailable} onBack={() => navigate(`/workspaces/${encodeURIComponent(workspaceId!)}/overview`)} organization={workspaceOrganization} organizations={organizations} session={session} signOut={signOut} workspace={workspace} />;
  }

  if (workspaceId) {
    return workspace ? (
      <SharedWorkspaceDetail
        accessToken={accessToken}
        apiAvailable={apiAvailable}
        onBack={() => navigate("/workspaces")}
        onWorkspaceDeleted={(deletedWorkspaceId) => {
          setWorkspaces((current) => current.filter((entry) => entry.workspace.id !== deletedWorkspaceId));
          navigate("/workspaces");
        }}
        onWorkspaceUpdated={(updatedWorkspace) => {
          setWorkspaces((current) => current.map((entry) => (
            entry.workspace.id === updatedWorkspace.id ? { ...entry, workspace: updatedWorkspace } : entry
          )));
        }}
        onNavigate={(section) => navigate(`/workspaces/${encodeURIComponent(workspaceId!)}/${section}`)}
        section={activeWorkspaceSection ?? "overview"}
        onOpenArtifact={(artifactId) => navigate(`/workspaces/${encodeURIComponent(workspaceId!)}/artifacts/${encodeURIComponent(artifactId)}`)}
        onOpenMemoryCard={(cardId) => navigate(`/workspaces/${encodeURIComponent(workspaceId!)}/memory-cards/${encodeURIComponent(cardId)}`)}
        organization={workspaceOrganization}
        session={session}
        signOut={signOut}
        workspace={workspace}
      />
    ) : (
      <SharedRouteNotFound apiAvailable={apiAvailable} onBack={() => navigate("/workspaces")} organizations={organizations} session={session} signOut={signOut} />
    );
  }

  if (pathname !== "/workspaces" && pathname !== "/") {
    return <SharedRouteNotFound apiAvailable={apiAvailable} onBack={() => navigate("/workspaces")} organizations={organizations} session={session} signOut={signOut} />;
  }

  return (
    <SharedWorkspaceHome
      accessToken={accessToken}
      apiAvailable={apiAvailable}
      organizations={organizations}
      session={session}
      setError={setError}
      setOrganizations={setOrganizations}
      setWorkspaces={setWorkspaces}
      signOut={signOut}
      onOpenWorkspace={(workspace) => navigate(`/workspaces/${encodeURIComponent(workspace.workspace.id)}/overview`)}
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
  apiAvailable,
  initialMode,
  initialError,
  onAuthenticated,
}: {
  apiAvailable: boolean | null;
  initialMode: AuthMode;
  initialError: string | null;
  onAuthenticated: (token: string) => void;
}) {
  const [mode, setMode] = useState<AuthMode>(initialMode);
  const [displayName, setDisplayName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(initialError);

  useEffect(() => {
    setMode(initialMode);
    setError(initialError);
  }, [initialError, initialMode]);

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
            <Button className={mode === "sign-in" ? "active" : ""} onClick={() => { navigate("/login"); setMode("sign-in"); setError(null); }} role="tab" type="button" variant="secondary">Sign in</Button>
            <Button className={mode === "sign-up" ? "active" : ""} onClick={() => { navigate("/register"); setMode("sign-up"); setError(null); }} role="tab" type="button" variant="secondary">Create account</Button>
          </div>
          <div className="shared-auth-heading">
            <p className="shared-eyebrow">{mode === "sign-in" ? "Welcome back" : "Start a shared workspace"}</p>
            <h2>{mode === "sign-in" ? "Continue to RepoMemo" : "Create your account"}</h2>
            <p>{mode === "sign-in" ? "Use the credentials registered with this server." : "Your account will be the initial owner of any organization you create."}</p>
          </div>
          <form className="shared-auth-form" onSubmit={submit}>
            {mode === "sign-up" ? <label>Display name<Input autoComplete="name" onChange={(event) => setDisplayName(event.target.value)} placeholder="Ada Lovelace" required value={displayName} /></label> : null}
            <label>Email<Input autoComplete="email" onChange={(event) => setEmail(event.target.value)} placeholder="you@company.com" required type="email" value={email} /></label>
            <label>Password<Input autoComplete={mode === "sign-up" ? "new-password" : "current-password"} minLength={12} onChange={(event) => setPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={password} /></label>
            {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
            <Button disabled={isSubmitting} type="submit" variant="main">
              {isSubmitting ? <Loader className="spin" size={17} /> : <ArrowRight size={17} />}
              {mode === "sign-in" ? "Sign in to shared mode" : "Create account and continue"}
            </Button>
          </form>
          <p className="shared-api-note">API {apiAvailable === true ? "healthy" : apiAvailable === false ? "unreachable" : "checking"} · <code>{sharedApiUrl}</code></p>
        </div>
      </section>
    </main>
  );
}

function SharedWorkspaceHome({
  accessToken,
  apiAvailable,
  organizations,
  session,
  setError,
  setOrganizations,
  setWorkspaces,
  signOut,
  onOpenWorkspace,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  organizations: Organization[];
  session: SharedSession;
  setError: (value: string | null) => void;
  setOrganizations: (value: Organization[] | ((current: Organization[]) => Organization[])) => void;
  setWorkspaces: (value: SharedWorkspace[] | ((current: SharedWorkspace[]) => SharedWorkspace[])) => void;
  signOut: () => void;
  onOpenWorkspace: (workspace: SharedWorkspace) => void;
  workspaces: SharedWorkspace[];
}) {
  const [organizationName, setOrganizationName] = useState("");
  const [workspaceName, setWorkspaceName] = useState("");
  const [organizationId, setOrganizationId] = useState(organizations[0]?.id ?? "");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

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

  return (
    <SharedAppShell
      apiAvailable={apiAvailable}
      session={session}
      signOut={signOut}
      sidebar={<OrganizationRail organizations={organizations} organizationId={organizationId} onSelectOrganization={setOrganizationId} />}
    >
        <div className="shared-page-content">
          <div className="shared-page-heading"><p className="shared-eyebrow">Shared workspaces</p><h1>{currentOrganization ? currentOrganization.name : "Set up your team"}</h1><p>{currentOrganization ? "Workspaces are server-authoritative and available only to their members." : "First create the organization that will own your team’s shared memory."}</p></div>
          {organizations.length === 0 ? <form className="shared-setup-form" onSubmit={createOrganization}><label>Organization name<Input autoFocus onChange={(event) => setOrganizationName(event.target.value)} placeholder="Engineering" required value={organizationName} /></label><Button disabled={isSubmitting} type="submit" variant="main">{isSubmitting ? <Loader className="spin" size={17} /> : <Plus size={17} />} Create organization</Button></form> : <>
            <form className="shared-create-workspace" onSubmit={createWorkspace}><div><strong>Create a workspace</strong><span>Start a bounded memory space for a repository, system, or initiative.</span></div><label className="sr-only" htmlFor="shared-workspace-name">Workspace name</label><Input id="shared-workspace-name" onChange={(event) => setWorkspaceName(event.target.value)} placeholder="Payments platform" required value={workspaceName} /><Button disabled={isSubmitting} type="submit" variant="main">{isSubmitting ? <Loader className="spin" size={17} /> : <Plus size={17} />} Create workspace</Button></form>
            <div className="shared-workspace-list">{workspaces.filter((workspace) => workspace.organization_id === organizationId).map((entry) => <article className="shared-workspace-row" key={entry.workspace.id}><span className="shared-workspace-icon"><Layers size={18} /></span><div><h2>{entry.workspace.name}</h2><p>Created {new Date(entry.workspace.created_at).toLocaleDateString()} · Your role: {entry.role}</p></div><Button aria-label={`Open ${entry.workspace.name}`} className="shared-workspace-open" onClick={() => onOpenWorkspace(entry)} type="button" variant="secondary"><ChevronRight size={18} /></Button></article>)}{workspaces.filter((workspace) => workspace.organization_id === organizationId).length === 0 ? <div className="shared-empty-state"><Layers size={26} /><strong>No workspaces yet</strong><span>Create the first shared workspace for {currentOrganization?.name}.</span></div> : null}</div>
          </>}
          {formError ? <p className="shared-form-error" role="alert">{formError}</p> : null}
          <p className="shared-boundary-note"><Shield size={15} /> Every workspace view is loaded through the JWT-protected API. This browser never falls back to local preview data.</p>
        </div>
    </SharedAppShell>
  );
}

function SharedAppShell({
  apiAvailable,
  children,
  session,
  sidebar,
  signOut,
}: {
  apiAvailable: boolean | null;
  children: ReactNode;
  session: SharedSession;
  sidebar: ReactNode;
  signOut: () => void;
}) {
  return (
    <main className="shared-home">
      <header className="shared-home-header">
        <div className="shared-brand"><span className="shared-brand-glyph"><Layers size={19} /></span><strong>RepoMemo</strong><span className="shared-mode-tag">Shared</span></div>
        <div className="shared-user-menu"><span>{session.user.display_name}</span><Button className="shared-user-signout" onClick={signOut} type="button" variant="secondary"><Logout size={16} /> Sign out</Button></div>
      </header>
      <div className="shared-home-frame">
        <aside className="shared-home-rail">{sidebar}<div className="shared-rail-footer"><Shield size={15} /><span>JWT active · API {apiAvailable === true ? "healthy" : apiAvailable === false ? "offline" : "checking"}</span></div></aside>
        <section className="shared-home-content">{children}</section>
      </div>
    </main>
  );
}

function OrganizationRail({
  organizations,
  organizationId,
  onSelectOrganization,
}: {
  organizations: Organization[];
  organizationId?: string;
  onSelectOrganization?: (organizationId: string) => void;
}) {
  return <><p className="shared-eyebrow">Organization</p>{organizations.length ? <div className="shared-organization-list">{organizations.map((organization) => <Button className={organization.id === organizationId ? "selected" : ""} disabled={!onSelectOrganization} key={organization.id} onClick={() => onSelectOrganization?.(organization.id)} type="button" variant="secondary"><Building size={16} /><span>{organization.name}</span></Button>)}</div> : <p className="shared-rail-empty">Create an organization to establish your team boundary.</p>}</>;
}

function WorkspaceRail({
  activeSection = "overview",
  onNavigate,
  organization,
  workspace,
}: {
  activeSection?: WorkspaceSection;
  onNavigate?: (section: WorkspaceSection) => void;
  organization?: Organization;
  workspace: SharedWorkspace;
}) {
  return <>
    <p className="shared-eyebrow">Organization</p>
    <div className="shared-rail-current"><Building size={16} /><span>{organization?.name ?? "Shared organization"}</span></div>
    <div className="shared-workspace-nav">
      <p className="shared-eyebrow">Workspace</p>
      <div className="shared-workspace-context"><Layers size={16} /><span>{workspace.workspace.name}</span></div>
      <nav aria-label="Workspace sections" className="shared-workspace-menu">
        <Button aria-current={activeSection === "overview" ? "page" : undefined} className={activeSection === "overview" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("overview")} type="button" variant="secondary"><Dashboard size={16} /> Overview</Button>
        <Button aria-current={activeSection === "evidence" ? "page" : undefined} className={activeSection === "evidence" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("evidence")} type="button" variant="secondary"><FileText size={16} /> Evidence</Button>
        <Button aria-current={activeSection === "retrieval" ? "page" : undefined} className={activeSection === "retrieval" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("retrieval")} type="button" variant="secondary"><Search size={16} /> Retrieval</Button>
        <Button aria-current={activeSection === "memory" ? "page" : undefined} className={activeSection === "memory" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("memory")} type="button" variant="secondary"><Book size={16} /> Memory</Button>
        <Button aria-current={activeSection === "people" ? "page" : undefined} className={activeSection === "people" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("people")} type="button" variant="secondary"><Users size={16} /> People</Button>
        <Button aria-current={activeSection === "activity" ? "page" : undefined} className={activeSection === "activity" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("activity")} type="button" variant="secondary"><Timeline size={16} /> Activity</Button>
      </nav>
      <p className="shared-sidebar-note">Navigate between the shared workspace views.</p>
    </div>
  </>;
}

function SharedWorkspaceDetail({
  accessToken,
  apiAvailable,
  onBack,
  onWorkspaceDeleted,
  onWorkspaceUpdated,
  onOpenArtifact,
  onOpenMemoryCard,
  onNavigate,
  organization,
  section,
  session,
  signOut,
  workspace,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  onBack: () => void;
  onWorkspaceDeleted: (workspaceId: string) => void;
  onWorkspaceUpdated: (workspace: SharedWorkspace["workspace"]) => void;
  onOpenArtifact: (artifactId: string) => void;
  onOpenMemoryCard: (cardId: string) => void;
  onNavigate: (section: WorkspaceSection) => void;
  organization?: Organization;
  section: WorkspaceSection;
  session: SharedSession;
  signOut: () => void;
  workspace: SharedWorkspace;
}) {
  const [overview, setOverview] = useState<WorkspaceOverview | null>(null);
  const [artifacts, setArtifacts] = useState<ArtifactSummary[]>([]);
  const [memoryCards, setMemoryCards] = useState<MemoryCardSummary[]>([]);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [query, setQuery] = useState("");
  const [noteTitle, setNoteTitle] = useState("");
  const [noteContent, setNoteContent] = useState("");
  const [uploadFile, setUploadFile] = useState<File | null>(null);
  const [memoryTitle, setMemoryTitle] = useState("");
  const [memoryBody, setMemoryBody] = useState("");
  const [memoryArtifactId, setMemoryArtifactId] = useState("");
  const [members, setMembers] = useState<WorkspaceMember[]>([]);
  const [activity, setActivity] = useState<WorkspaceActivityEvent[]>([]);
  const [capabilities, setCapabilities] = useState<WorkspaceCapabilities | null>(null);
  const [aiOverview, setAiOverview] = useState<WorkspaceAiOverview | null>(null);
  const [aiProviders, setAiProviders] = useState<SharedAiProviderSettings[]>([]);
  const [memberEmail, setMemberEmail] = useState("");
  const [memberRole, setMemberRole] = useState<WorkspaceRole>("member");
  const [workspaceName, setWorkspaceName] = useState(workspace.workspace.name);
  const [providerId, setProviderId] = useState("");
  const [providerType, setProviderType] = useState<"ollama" | "openrouter">("ollama");
  const [providerName, setProviderName] = useState("Local Ollama");
  const [providerBaseUrl, setProviderBaseUrl] = useState("http://127.0.0.1:11434");
  const [providerModel, setProviderModel] = useState("llama3.2");
  const [providerApiKey, setProviderApiKey] = useState("");
  const [cloudContentAcknowledged, setCloudContentAcknowledged] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const canWrite = capabilities?.can_write_content ?? false;
  const canManageMembers = capabilities?.can_manage_members ?? false;
  const canAssignAdmin = capabilities?.can_assign_admin ?? false;
  const canManageWorkspace = capabilities?.can_manage_workspace ?? false;
  const isEvidenceView = section === "evidence";
  const isRetrievalView = section === "retrieval";
  const isMemoryView = section === "memory";
  const isPeopleView = section === "people";
  const isActivityView = section === "activity";

  async function load() {
    setIsLoading(true);
    setError(null);
    try {
      const [nextOverview, nextArtifacts, nextMemory, nextMembers, nextCapabilities] = await Promise.all([
        getSharedWorkspaceOverview(accessToken, workspace.workspace.id),
        listSharedArtifacts(accessToken, workspace.workspace.id),
        listSharedMemoryCards(accessToken, workspace.workspace.id),
        listSharedWorkspaceMembers(accessToken, workspace.workspace.id),
        getSharedWorkspaceCapabilities(accessToken, workspace.workspace.id),
      ]);
      setOverview(nextOverview);
      setArtifacts(nextArtifacts);
      setMemoryCards(nextMemory);
      setMembers(nextMembers);
      setCapabilities(nextCapabilities);
      if (nextCapabilities.can_manage_workspace) {
        setAiProviders(await listSharedWorkspaceAiProviders(accessToken, workspace.workspace.id));
      } else {
        setAiProviders([]);
      }
      if (isActivityView) {
        setActivity(await listSharedWorkspaceActivity(accessToken, workspace.workspace.id));
      }
    } catch (requestError) {
      setError(apiMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }

  useEffect(() => { void load(); }, [accessToken, isActivityView, workspace.workspace.id]);
  useEffect(() => { setWorkspaceName(workspace.workspace.name); }, [workspace.workspace.name]);
  useEffect(() => {
    const provider = aiProviders.find((entry) => entry.enabled) ?? aiProviders[0];
    if (!provider) return;
    setProviderId(provider.id);
    setProviderType(provider.provider_type);
    setProviderName(provider.name);
    setProviderBaseUrl(provider.base_url ?? "");
    setProviderModel(provider.model ?? "");
  }, [aiProviders]);

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
        citations: memoryArtifactId ? artifactCitation(artifacts, memoryArtifactId) : [],
      });
      setMemoryTitle(""); setMemoryBody(""); setMemoryArtifactId("");
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function submitUpload(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    if (!uploadFile) return;
    if (uploadFile.size > 10 * 1024 * 1024) {
      setError("Files must be 10 MiB or smaller.");
      return;
    }
    setIsSubmitting(true); setError(null);
    try {
      await uploadSharedArtifact(accessToken, workspace.workspace.id, uploadFile);
      setUploadFile(null);
      form.reset();
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

  async function saveAiProvider(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      const saved = await saveSharedWorkspaceAiProvider(accessToken, workspace.workspace.id, {
        id: providerId || undefined,
        providerType,
        name: providerName,
        baseUrl: providerBaseUrl,
        model: providerModel,
        apiKey: providerApiKey || undefined,
        enabled: true,
        cloudContentAcknowledged,
      });
      setAiProviders((current) => [saved, ...current.filter((entry) => entry.id !== saved.id)]);
      setProviderId(saved.id);
      setProviderApiKey("");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function selectProviderType(value: "ollama" | "openrouter") {
    setProviderType(value);
    if (value === "openrouter") {
      if (!providerBaseUrl || providerBaseUrl === "http://127.0.0.1:11434") setProviderBaseUrl("https://openrouter.ai/api/v1");
      if (!providerModel || providerModel === "llama3.2") setProviderModel("openai/gpt-4o-mini");
    } else {
      if (!providerBaseUrl || providerBaseUrl === "https://openrouter.ai/api/v1") setProviderBaseUrl("http://127.0.0.1:11434");
      if (!providerModel || providerModel === "openai/gpt-4o-mini") setProviderModel("llama3.2");
    }
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
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeMember(userId: string) {
    setIsSubmitting(true); setError(null);
    try { await removeSharedWorkspaceMember(accessToken, workspace.workspace.id, userId); await load(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
    <SharedAppShell apiAvailable={apiAvailable} session={session} signOut={signOut} sidebar={<WorkspaceRail activeSection={section} onNavigate={onNavigate} organization={organization} workspace={workspace} />}>
      <section className="shared-detail-shell" aria-busy={isLoading}>
        <div className="shared-detail-heading">
          <div><p className="shared-eyebrow">{isEvidenceView ? "Evidence" : isRetrievalView ? "Retrieval" : isMemoryView ? "Team memory" : isPeopleView ? "Workspace access" : isActivityView ? "Workspace history" : `Server workspace · ${workspace.role}`}</p><h1>{isEvidenceView ? "Evidence ledger" : isRetrievalView ? "Retrieve evidence" : isMemoryView ? "Durable team memory" : isPeopleView ? "People" : isActivityView ? "Activity" : workspace.workspace.name}</h1><p>{isEvidenceView ? "Store notes and files in the workspace, then index them for retrieval." : isRetrievalView ? "Search indexed workspace evidence and inspect the source context behind every result." : isMemoryView ? "Capture concise facts and decisions that should outlive the current investigation." : isPeopleView ? "See who can access this workspace and manage roles when your membership allows it." : isActivityView ? "A durable record of shared workspace changes, including evidence, memory, indexing, and membership updates." : "Artifacts, search results, and durable team memory are all retrieved through the protected shared API."}</p></div>
          <div className="shared-detail-actions"><Button className="shared-back-button" onClick={onBack} type="button" variant="secondary"><ArrowLeft size={16} /> All workspaces</Button><Button disabled={isLoading} onClick={() => void load()} type="button" variant="secondary"><Refresh size={16} /> Refresh</Button>{canWrite ? <Button disabled={isSubmitting || isLoading} onClick={() => void runIndex()} type="button" variant="main">{isSubmitting ? <Loader className="spin" size={16} /> : <Layers size={16} />} {isEvidenceView ? "Index evidence" : "Index workspace"}</Button> : null}</div>
        </div>
        {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
        {!isEvidenceView && !isRetrievalView && !isMemoryView && !isPeopleView && !isActivityView ? <div className="shared-evidence-summary">
          <span><strong>{overview?.artifact_count ?? "—"}</strong> artifacts</span><span><strong>{overview?.chunk_count ?? "—"}</strong> indexed chunks</span><span><strong>{overview?.memory_card_count ?? "—"}</strong> memory cards</span>
        </div> : null}
        {section === "overview" ? <section className="shared-ai-overview" aria-live="polite">
          <div className="shared-panel-heading"><div><Brain size={18} /><h2>AI workspace overview</h2></div>{aiOverview?.provider_name ? <span>{aiOverview.provider_name}</span> : null}</div>
          {aiOverview?.summary_markdown ? <div className="shared-ai-overview-body">{aiOverview.summary_markdown}</div> : <p className="shared-muted-copy">Generate a concise project briefing from indexed evidence. RepoMemo only uses an enabled provider configured for this workspace and returns the source citations with the result.</p>}
          {aiOverview?.warnings.length ? <div className="shared-ai-overview-warning">{aiOverview.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}
          {aiOverview?.citations.length ? <div className="shared-ai-citations"><strong>Evidence used</strong>{aiOverview.citations.map((citation) => <span key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}`}>{citation.title} · {citation.path}{citation.start_line ? ` · line ${citation.start_line}` : ""}</span>)}</div> : null}
          <Button disabled={isSubmitting || isLoading || !capabilities?.can_generate_ai_overview} onClick={() => void generateAiOverview()} type="button" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Brain size={16} />}{aiOverview?.summary_markdown ? "Refresh AI overview" : "Generate AI overview"}</Button>
        </section> : null}
        <div className={`shared-detail-grid${isEvidenceView ? " evidence-only" : isRetrievalView ? " retrieval-only" : isMemoryView ? " memory-only" : isPeopleView ? " people-only" : isActivityView ? " activity-only" : ""}`}>
          <section className="shared-detail-panel" hidden={isRetrievalView || isMemoryView || isPeopleView || isActivityView}>
            <div className="shared-panel-heading"><div><FileText size={18} /><h2>Evidence ledger</h2></div><span>{artifacts.length} records</span></div>
            {artifacts.length ? <div className="shared-artifact-list">{artifacts.map((artifact) => <article key={artifact.id}><Button className="shared-record-link" onClick={() => onOpenArtifact(artifact.id)} type="button" variant="secondary"><div><strong>{artifact.title}</strong><span>{artifact.path} · {artifact.indexed_at ? "Indexed" : "Not indexed"}</span></div><small>{artifact.language ?? artifact.artifact_type}</small></Button></article>)}</div> : <div className="shared-empty-state"><FileText size={25} /><strong>No shared evidence yet</strong><span>Add a pasted note below, then index it when you are ready to search.</span></div>}
            {canWrite ? <><form className="shared-note-form" onSubmit={addNote}><h3>Add shared note</h3><Input onChange={(event) => setNoteTitle(event.target.value)} placeholder="Decision or implementation note" required value={noteTitle} /><Textarea onChange={(event) => setNoteContent(event.target.value)} placeholder="Paste Markdown, code context, or a meeting note…" required value={noteContent} /><Button disabled={isSubmitting} type="submit" variant="main"><Plus size={16} /> Store evidence</Button></form><form className="shared-upload-form" onSubmit={submitUpload}><div><strong>Upload a file</strong><span>Markdown, text, code, or supported image · up to 10 MiB</span></div><label className="shared-upload-picker"><input accept=".md,.mdx,.txt,.rs,.ts,.tsx,.js,.jsx,.py,.json,.toml,.yaml,.yml,.sql,.html,.css,.sh,.ps1,.png,.jpg,.jpeg,.gif,.webp,.svg,.bmp" aria-label="Upload a shared artifact" className="shared-upload-native-input" onChange={(event) => setUploadFile(event.target.files?.[0] ?? null)} required type="file" /><span className="shared-upload-picker-icon"><Upload size={18} /></span><span className="shared-upload-picker-copy"><strong>{uploadFile?.name ?? "Choose a shared file"}</strong><span>{uploadFile ? `${formatFileSize(uploadFile.size)} · ready to upload` : "Markdown, text, code, or a supported image"}</span></span><span className="shared-upload-picker-action">Browse</span></label><Button disabled={isSubmitting || !uploadFile} type="submit" variant="secondary"><Upload size={16} /> Upload</Button></form></> : <p className="shared-readonly-note"><Shield size={15} /> Your viewer membership can inspect shared evidence but cannot change it.</p>}
          </section>
          <aside className="shared-detail-panel shared-retrieval-panel" hidden={isEvidenceView}>
            <div className="shared-panel-heading" hidden={isMemoryView || isPeopleView || isActivityView}><div><Search size={18} /><h2>Retrieve</h2></div></div>
            <form className="shared-search-form" hidden={isMemoryView || isPeopleView || isActivityView} onSubmit={runSearch}><Input onChange={(event) => setQuery(event.target.value)} placeholder="Search indexed evidence" value={query} /><Button disabled={isSubmitting} type="submit" variant="secondary">Search</Button></form>
            <div hidden={isMemoryView || isPeopleView || isActivityView}>{results.length ? <div className="shared-search-results">{results.map((result) => <article key={result.chunk_id}><strong>{result.title}</strong><p>{result.snippet}</p><span>{result.path}{result.start_line ? ` · line ${result.start_line}` : ""}</span></article>)}</div> : <p className="shared-muted-copy">Index one or more artifacts, then search the evidence base from here.</p>}</div>
            <div className="shared-memory-section" hidden={isRetrievalView || isPeopleView || isActivityView}><div className="shared-panel-heading"><div><Shield size={18} /><h2>Team memory</h2></div><span>{memoryCards.length}</span></div>{memoryCards.length ? <div className="shared-memory-list">{memoryCards.map((card) => <article key={card.id}><Button className="shared-record-link" onClick={() => onOpenMemoryCard(card.id)} type="button" variant="secondary"><strong>{card.title}</strong><span>{card.body_excerpt}</span></Button></article>)}</div> : <p className="shared-muted-copy">No durable memory cards yet.</p>}{canWrite ? <form className="shared-memory-form" onSubmit={addMemory}><Input onChange={(event) => setMemoryTitle(event.target.value)} placeholder="Memory title" required value={memoryTitle} /><Textarea onChange={(event) => setMemoryBody(event.target.value)} placeholder="A concise durable fact…" required value={memoryBody} /><label>Evidence link<Dropdown aria-label="Evidence link" onValueChange={(value) => setMemoryArtifactId(value === "__none__" ? "" : value)} options={[{ label: "No direct artifact link", value: "__none__" }, ...artifacts.map((artifact) => ({ label: artifact.title, value: artifact.id }))]} value={memoryArtifactId || "__none__"} /></label><Button disabled={isSubmitting} type="submit" variant="secondary">Save memory</Button></form> : null}</div>
            <div className="shared-members-section" hidden={isRetrievalView || isMemoryView || isActivityView}>
              <div className="shared-panel-heading"><div><Users size={18} /><h2>Workspace team</h2></div><span>{members.length}</span></div>
              <p className="shared-muted-copy">{canManageMembers ? "Add people who already have a RepoMemo account, then choose what they can do here." : "Your workspace role does not allow membership changes."}</p>
              <div className="shared-capabilities" aria-label="Your workspace capabilities">
                <strong>Your {capabilities?.role ?? workspace.role} access</strong>
                <span>Read evidence, retrieval, memory, exports, and activity.</span>
                <span>{canWrite ? "Create, edit, delete, and index shared evidence and memory." : "View-only: shared content cannot be changed."}</span>
                {canManageMembers ? <span>Manage workspace membership{canAssignAdmin ? " and assign administrator roles." : ", except administrator roles."}</span> : null}
                {canManageWorkspace ? <span>Rename or permanently delete this workspace.</span> : null}
              </div>
              <div className="shared-member-list">
                {members.map((member) => <article className="shared-member-row" key={member.user.id}>
                  <div><strong>{member.user.display_name}</strong><span>{member.user.email ?? "No email"}</span></div>
                  <span className="shared-member-role">{member.role}</span>
                  {canManageMembers && member.role !== "owner" ? <Button className="shared-member-remove" disabled={isSubmitting} onClick={() => void removeMember(member.user.id)} type="button" variant="secondary">Remove</Button> : null}
                </article>)}
              </div>
              {canManageMembers ? <form className="shared-member-form" onSubmit={saveMember}>
                <Input aria-label="Member email" onChange={(event) => setMemberEmail(event.target.value)} placeholder="person@example.com" required type="email" value={memberEmail} />
                <Dropdown aria-label="Member role" onValueChange={(value) => setMemberRole(value as WorkspaceRole)} options={[{ label: "Viewer", value: "viewer" }, { label: "Member", value: "member" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} value={memberRole} />
                <Button disabled={isSubmitting} type="submit" variant="secondary">Add / update</Button>
              </form> : null}
              {canManageWorkspace ? <section className="shared-workspace-admin">
                <div><h3>Workspace controls</h3><p>Only the owner can rename or permanently delete this shared workspace.</p></div>
                <form onSubmit={renameWorkspace}><Input aria-label="Workspace name" onChange={(event) => setWorkspaceName(event.target.value)} required value={workspaceName} /><Button disabled={isSubmitting} type="submit" variant="secondary"><Pencil size={16} /> Rename</Button></form>
                <Button className="shared-danger-action" disabled={isSubmitting} onClick={() => void removeWorkspace()} type="button" variant="secondary"><Trash size={16} /> Delete workspace</Button>
                <form className="shared-ai-provider-form" onSubmit={saveAiProvider}>
                  <div><h3>AI overview provider</h3><p>Configure one explicit workspace provider. Credentials are sent only to this protected server and are never returned to the browser after saving.</p></div>
                  <label>Provider<Dropdown aria-label="AI provider" onValueChange={(value) => selectProviderType(value as "ollama" | "openrouter")} options={[{ label: "Ollama (local)", value: "ollama" }, { label: "OpenRouter (cloud)", value: "openrouter" }]} value={providerType} /></label>
                  <label>Provider name<Input onChange={(event) => setProviderName(event.target.value)} required value={providerName} /></label>
                  <label>Base URL<Input onChange={(event) => setProviderBaseUrl(event.target.value)} placeholder={providerType === "ollama" ? "http://127.0.0.1:11434" : "https://openrouter.ai/api/v1"} value={providerBaseUrl} /></label>
                  <label>Chat model<Input onChange={(event) => setProviderModel(event.target.value)} placeholder={providerType === "ollama" ? "llama3.2" : "openai/gpt-4o-mini"} required value={providerModel} /></label>
                  {providerType === "openrouter" ? <><label>API key<Input autoComplete="off" onChange={(event) => setProviderApiKey(event.target.value)} placeholder={providerId ? "Leave blank to keep the saved key" : "Required to enable cloud AI"} type="password" value={providerApiKey} /></label><label className="shared-ai-provider-toggle"><input checked={cloudContentAcknowledged} onChange={(event) => setCloudContentAcknowledged(event.target.checked)} type="checkbox" /> I understand that generating an overview sends the cited workspace excerpts to this cloud provider.</label></> : null}
                  <Button disabled={isSubmitting} type="submit" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Brain size={16} />} Save AI provider</Button>
                </form>
              </section> : null}
            </div>
            <div className="shared-activity-section" hidden={!isActivityView}>
              <div className="shared-panel-heading"><div><Timeline size={18} /><h2>Recent activity</h2></div><span>{activity.length}</span></div>
              {activity.length ? <div className="shared-activity-list">{activity.map((event) => <article key={event.id}><div><strong>{event.summary}</strong><span>{event.actor?.display_name ?? "System"} · {event.action.replace(/_/g, " ")}</span></div><time dateTime={event.created_at}>{formatActivityTime(event.created_at)}</time></article>)}</div> : <div className="shared-empty-state"><Timeline size={25} /><strong>No recorded activity yet</strong><span>New workspace changes will appear here.</span></div>}
            </div>
          </aside>
        </div>
      </section>
    </SharedAppShell>
  );
}

function SharedArtifactDetail({
  accessToken,
  apiAvailable,
  artifactId,
  onBack,
  organization,
  session,
  signOut,
  workspace,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  artifactId: string;
  onBack: () => void;
  organization?: Organization;
  session: SharedSession;
  signOut: () => void;
  workspace: SharedWorkspace;
}) {
  const [artifact, setArtifact] = useState<ArtifactDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isIndexing, setIsIndexing] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isMutating, setIsMutating] = useState(false);
  const [title, setTitle] = useState("");
  const canWrite = workspace.role !== "viewer";

  async function load() {
    setIsLoading(true); setError(null);
    try { setArtifact(await getSharedArtifact(accessToken, artifactId)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsLoading(false); }
  }

  useEffect(() => { void load(); }, [accessToken, artifactId]);
  useEffect(() => { setTitle(artifact?.summary.title ?? ""); }, [artifact?.summary.title]);

  async function indexArtifact() {
    setIsIndexing(true); setError(null);
    try { await indexSharedArtifact(accessToken, artifactId); await load(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsIndexing(false); }
  }

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

  return (
    <SharedRecordLayout activeSection="evidence" apiAvailable={apiAvailable} backLabel="Workspace" onBack={onBack} organization={organization} session={session} signOut={signOut} title={artifact?.summary.title ?? "Artifact"} subtitle={artifact?.summary.path ?? "Loading protected evidence…"} workspace={workspace}>
      <div className="shared-detail-actions"><Button disabled={isLoading} onClick={() => void load()} type="button" variant="secondary"><Refresh size={16} /> Refresh</Button>{canWrite ? <><Button disabled={isLoading || isMutating} onClick={() => setIsEditing((value) => !value)} type="button" variant="secondary"><Pencil size={16} /> {isEditing ? "Cancel edit" : "Edit"}</Button><Button className="shared-danger-action" disabled={isLoading || isMutating} onClick={() => void removeArtifact()} type="button" variant="secondary"><Trash size={16} /> Delete</Button><Button disabled={isLoading || isIndexing || isMutating} onClick={() => void indexArtifact()} type="button" variant="main">{isIndexing ? <Loader className="spin" size={16} /> : <Layers size={16} />} Index artifact</Button></> : null}</div>
      {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
      <div className="shared-record-meta"><span>{artifact?.summary.artifact_type ?? "artifact"}</span><span>{artifact?.summary.language ?? "Unspecified language"}</span><span>{artifact?.summary.indexed_at ? "Indexed" : "Not indexed"}</span></div>
      {isEditing ? <form className="shared-record-edit-form" onSubmit={saveArtifact}><label>Evidence title<Input onChange={(event) => setTitle(event.target.value)} required value={title} /></label><Button disabled={isMutating} type="submit" variant="main">{isMutating ? <Loader className="spin" size={16} /> : <Pencil size={16} />} Save evidence</Button></form> : null}
      <section className="shared-record-panel"><h2>Stored content</h2>{artifact?.content_preview ? <pre className="shared-content-preview">{artifact.content_preview}</pre> : <p className="shared-muted-copy">This artifact has no text preview available.</p>}</section>
      <section className="shared-record-panel"><h2>Indexed evidence</h2>{artifact?.chunks.length ? <div className="shared-chunk-list">{artifact.chunks.map((chunk) => <article key={chunk.id}><span>{chunk.start_line ? `Lines ${chunk.start_line}${chunk.end_line && chunk.end_line !== chunk.start_line ? `–${chunk.end_line}` : ""}` : "Stored chunk"}</span><p>{chunk.text}</p></article>)}</div> : <p className="shared-muted-copy">Index this artifact to create retrievable evidence chunks.</p>}</section>
    </SharedRecordLayout>
  );
}

function SharedMemoryCardDetail({
  accessToken,
  apiAvailable,
  cardId,
  onBack,
  organization,
  session,
  signOut,
  workspace,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  cardId: string;
  onBack: () => void;
  organization?: Organization;
  session: SharedSession;
  signOut: () => void;
  workspace: SharedWorkspace;
}) {
  const [card, setCard] = useState<MemoryCardDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isExporting, setIsExporting] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isMutating, setIsMutating] = useState(false);
  const [title, setTitle] = useState("");
  const [bodyMarkdown, setBodyMarkdown] = useState("");
  const [source, setSource] = useState("");
  const canWrite = workspace.role !== "viewer";

  async function load() {
    setIsLoading(true); setError(null);
    try { setCard(await getSharedMemoryCard(accessToken, cardId)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsLoading(false); }
  }

  useEffect(() => { void load(); }, [accessToken, cardId]);
  useEffect(() => {
    setTitle(card?.card.title ?? "");
    setBodyMarkdown(card?.card.body_markdown ?? "");
    setSource(card?.card.source ?? "");
  }, [card?.card.body_markdown, card?.card.source, card?.card.title]);

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
      await updateSharedMemoryCard(accessToken, cardId, { title, bodyMarkdown, source });
      setIsEditing(false);
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function removeCard() {
    if (!window.confirm(`Delete ${card?.card.title ?? "this memory card"}? This cannot be undone.`)) return;
    setIsMutating(true); setError(null);
    try { await deleteSharedMemoryCard(accessToken, cardId); onBack(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  return (
    <SharedRecordLayout activeSection="memory" apiAvailable={apiAvailable} backLabel="Workspace" onBack={onBack} organization={organization} session={session} signOut={signOut} title={card?.card.title ?? "Memory card"} subtitle={card ? `Source: ${card.card.source}` : "Loading durable team memory…"} workspace={workspace}>
      <div className="shared-detail-actions"><Button disabled={isLoading} onClick={() => void load()} type="button" variant="secondary"><Refresh size={16} /> Refresh</Button>{canWrite ? <><Button disabled={isLoading || isMutating} onClick={() => setIsEditing((value) => !value)} type="button" variant="secondary"><Pencil size={16} /> {isEditing ? "Cancel edit" : "Edit"}</Button><Button className="shared-danger-action" disabled={isLoading || isMutating} onClick={() => void removeCard()} type="button" variant="secondary"><Trash size={16} /> Delete</Button></> : null}<Button disabled={isLoading || isExporting || isMutating} onClick={() => void exportCard()} type="button" variant="main">{isExporting ? <Loader className="spin" size={16} /> : <FileText size={16} />} Export Markdown</Button></div>
      {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
      <section className="shared-record-panel"><h2>Durable statement</h2>{isEditing ? <form className="shared-record-edit-form" onSubmit={saveCard}><label>Title<Input onChange={(event) => setTitle(event.target.value)} required value={title} /></label><label>Source<Input onChange={(event) => setSource(event.target.value)} required value={source} /></label><label>Statement<Textarea onChange={(event) => setBodyMarkdown(event.target.value)} required value={bodyMarkdown} /></label><Button disabled={isMutating} type="submit" variant="main">{isMutating ? <Loader className="spin" size={16} /> : <Pencil size={16} />} Save memory</Button></form> : <div className="shared-memory-body">{card?.card.body_markdown ?? ""}</div>}</section>
      <section className="shared-record-panel"><h2>Linked evidence</h2>{card?.evidence.length ? <div className="shared-evidence-links">{card.evidence.map((evidence) => <article key={evidence.link_id}><strong>{evidence.title ?? "Untitled evidence"}</strong><span>{evidence.path ?? evidence.target_id}{evidence.start_line ? ` · line ${evidence.start_line}` : ""}</span></article>)}</div> : <p className="shared-muted-copy">This memory card currently has no linked evidence.</p>}</section>
    </SharedRecordLayout>
  );
}

function SharedRecordLayout({
  activeSection,
  apiAvailable,
  backLabel,
  children,
  onBack,
  organization,
  session,
  signOut,
  subtitle,
  title,
  workspace,
}: {
  activeSection: WorkspaceSection;
  apiAvailable: boolean | null;
  backLabel: string;
  children: ReactNode;
  onBack: () => void;
  organization?: Organization;
  session: SharedSession;
  signOut: () => void;
  subtitle: string;
  title: string;
  workspace: SharedWorkspace;
}) {
  return (
    <SharedAppShell apiAvailable={apiAvailable} session={session} signOut={signOut} sidebar={<WorkspaceRail activeSection={activeSection} organization={organization} workspace={workspace} />}>
      <section className="shared-detail-shell">
        <div className="shared-detail-heading"><div><p className="shared-eyebrow">Protected record</p><h1>{title}</h1><p className="shared-record-subtitle">{subtitle}</p></div><Button className="shared-back-button" onClick={onBack} type="button" variant="secondary"><ArrowLeft size={16} /> {backLabel}</Button></div>
        <div className="shared-record-content">{children}</div>
      </section>
    </SharedAppShell>
  );
}

function SharedRouteNotFound({
  apiAvailable,
  onBack,
  organization,
  organizations,
  session,
  signOut,
  workspace,
}: {
  apiAvailable: boolean | null;
  onBack: () => void;
  organization?: Organization;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspace?: SharedWorkspace;
}) {
  return (
    <SharedAppShell apiAvailable={apiAvailable} session={session} signOut={signOut} sidebar={workspace ? <WorkspaceRail organization={organization} workspace={workspace} /> : <OrganizationRail organizations={organizations} />}>
      <section className="shared-page-content shared-route-page" aria-label="Unknown shared route">
        <p className="shared-eyebrow">Shared workspace</p>
        <h1>This address has no shared view.</h1>
        <p className="shared-intro-copy">The requested route is not available, or the workspace is not in your current signed-in membership.</p>
        <Button className="shared-route-action" onClick={onBack} type="button" variant="main"><ArrowLeft size={17} /> Go to workspaces</Button>
      </section>
    </SharedAppShell>
  );
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatActivityTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function artifactCitation(artifacts: ArtifactSummary[], artifactId: string) {
  const artifact = artifacts.find((entry) => entry.id === artifactId);
  if (!artifact) return [];
  return [{
    artifact_id: artifact.id,
    chunk_id: null,
    title: artifact.title,
    path: artifact.path,
    start_line: null,
    end_line: null,
    confidence: null,
  }];
}

function apiMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) return String((error as SharedApiError).message);
  return "The shared API could not be reached. Confirm that the server is running and try again.";
}
