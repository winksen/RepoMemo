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
  IconBell as Bell,
  IconChevronRight as ChevronRight,
  IconChartBar as Chart,
  IconFileText as FileText,
  IconFilter as Filter,
  IconLayoutGrid as Grid,
  IconLayoutDashboard as Dashboard,
  IconBook2 as Book,
  IconChecklist as Checklist,
  IconMessageCircle as MessageCircle,
  IconKey as Key,
  IconList as List,
  IconLoader2 as Loader,
  IconLogout as Logout,
  IconMoon as Moon,
  IconRefresh as Refresh,
  IconSearch as Search,
  IconSettings as Settings,
  IconPlus as Plus,
  IconPencil as Pencil,
  IconShieldLock as Shield,
  IconStack2 as Layers,
  IconTimeline as Timeline,
  IconTrash as Trash,
  IconUsers as Users,
  IconUpload as Upload,
  IconUserCircle as UserCircle,
  IconSun as Sun,
} from "@tabler/icons-react";
import {
  askSharedWorkspace,
  createSharedOrganization,
  createSharedArtifactComment,
  createSharedCollaborationTask,
  changeSharedPassword,
  createSharedMemoryCard,
  createSharedTextArtifact,
  createSharedWorkspace,
  deleteSharedArtifact,
  deleteSharedArtifactComment,
  deleteSharedCollaborationTask,
  deleteSharedMemoryCard,
  deleteSharedWorkspace,
  exportSharedMemoryCard,
  getSharedArtifact,
  getSharedArtifactLifecycle,
  getSharedHealth,
  getSharedProfile,
  getSharedWorkspaceCapabilities,
  getSharedWorkspaceActivityCalendar,
  getSharedWorkspaceMetrics,
  getSharedRetrievalFacets,
  getSharedMemoryCard,
  getSharedSession,
  getSharedWorkspaceOverview,
  indexSharedWorkspace,
  indexSharedArtifact,
  listSharedArtifacts,
  listSharedArtifactComments,
  listSharedArtifactLifecycleEvents,
  listSharedCollaborationTasks,
  listSharedWorkspaceAiProviders,
  listSharedWorkspaceActivity,
  listSharedWorkspaceMembers,
  listSharedMemoryCards,
  listSharedOrganizations,
  listSharedProfileTasks,
  listSharedNotifications,
  listSharedWorkspaces,
  loginSharedUser,
  generateSharedWorkspaceAiOverview,
  registerSharedUser,
  querySharedArtifacts,
  searchSharedWorkspace,
  searchSharedMemoryCards,
  saveSharedWorkspaceAiProvider,
  testSharedWorkspaceAiProvider,
  removeSharedWorkspaceMember,
  sharedApiUrl,
  upsertSharedWorkspaceMember,
  uploadSharedArtifact,
  updateSharedArtifact,
  updateSharedArtifactLifecycle,
  markAllSharedNotificationsRead,
  markSharedNotificationRead,
  updateSharedArtifactComment,
  updateSharedCollaborationTask,
  updateSharedProfile,
  updateSharedMemoryCard,
  updateSharedWorkspace,
  type SharedApiError,
} from "./lib/sharedApi";
import type {
  AskAnswer,
  ArtifactSummary,
  ArtifactDetail,
  ArtifactComment,
  ArtifactLifecycle,
  ArtifactLifecycleEvent,
  ArtifactLifecycleStatus,
  ArtifactType,
  MemoryCardDetail,
  MemoryCardSummary,
  Organization,
  ProviderTestResult,
  RetrievalFacets,
  SearchResult,
  SharedAiProviderSettings,
  SharedSession,
  SharedUser,
  SharedWorkspace,
  WorkspaceMember,
  CollaborationTask,
  CollaborationTaskPriority,
  CollaborationTaskStatus,
  WorkspaceActivityEvent,
  WorkspaceActivityCalendar,
  WorkspaceAiOverview,
  WorkspaceCapabilities,
  WorkspaceRole,
  WorkspaceOverview,
  WorkspaceMetrics,
  WorkspaceMetricBreakdown,
  UserProfile,
  SharedNotification,
} from "./types";
import { Button } from "./components/ui/button";
import { Dropdown } from "./components/ui/dropdown";
import { Input } from "./components/ui/input";
import { Textarea } from "./components/ui/textarea";

const SESSION_STORAGE_KEY = "repomemo.shared.access-token";
const THEME_STORAGE_KEY = "repomemo.theme";

type AuthMode = "sign-in" | "sign-up";
type PageState = "restoring" | "unauthenticated" | "ready" | "error";
type WorkspaceSection = "overview" | "evidence" | "retrieval" | "memory" | "tasks" | "people" | "activity" | "settings";
type Theme = "light" | "dark";
type ArtifactViewMode = "grid" | "list";

const WORKSPACE_SECTIONS: WorkspaceSection[] = ["overview", "evidence", "retrieval", "memory", "tasks", "people", "activity", "settings"];
const TASK_COLUMNS: Array<{ status: CollaborationTaskStatus; label: string }> = [
  { status: "open", label: "Open" },
  { status: "in_progress", label: "In progress" },
  { status: "blocked", label: "Blocked" },
  { status: "done", label: "Done" },
];

function initialTheme(): Theme {
  const storedTheme = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (storedTheme === "light" || storedTheme === "dark") return storedTheme;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

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
    document.documentElement.dataset.theme = initialTheme();
  }, []);

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

  if (pathname === "/profile") {
    return <SharedProfile accessToken={accessToken} apiAvailable={apiAvailable} organizations={organizations} onSessionUserUpdated={(user) => setSession((current) => current ? { ...current, user } : current)} session={session} signOut={signOut} />;
  }

  if (pathname === "/notifications") {
    return <SharedNotifications accessToken={accessToken} apiAvailable={apiAvailable} organizations={organizations} session={session} signOut={signOut} />;
  }

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

function SharedProfile({
  accessToken,
  apiAvailable,
  organizations,
  onSessionUserUpdated,
  session,
  signOut,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  organizations: Organization[];
  onSessionUserUpdated: (user: SharedUser) => void;
  session: SharedSession;
  signOut: () => void;
}) {
  const [profile, setProfile] = useState<UserProfile | null>(null);
  const [assignedTasks, setAssignedTasks] = useState<CollaborationTask[]>([]);
  const [displayName, setDisplayName] = useState(session.user.display_name);
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);

  async function load() {
    setIsLoading(true); setError(null);
    try {
      const [nextProfile, nextAssignedTasks] = await Promise.all([
        getSharedProfile(accessToken),
        listSharedProfileTasks(accessToken),
      ]);
      setProfile(nextProfile);
      setAssignedTasks(nextAssignedTasks);
      setDisplayName(nextProfile.user.display_name);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsLoading(false); }
  }

  useEffect(() => { void load(); }, [accessToken]);

  async function saveProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null); setNotice(null);
    try {
      const user = await updateSharedProfile(accessToken, displayName);
      onSessionUserUpdated(user);
      setProfile((current) => current ? { ...current, user, updated_at: new Date().toISOString() } : current);
      setNotice("Profile name updated.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function changePassword(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (newPassword !== confirmPassword) { setError("New password confirmation does not match."); return; }
    setIsSubmitting(true); setError(null); setNotice(null);
    try {
      await changeSharedPassword(accessToken, { currentPassword, newPassword });
      setCurrentPassword(""); setNewPassword(""); setConfirmPassword("");
      setNotice("Password changed. Your current session remains active.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  const initials = (profile?.user.display_name ?? session.user.display_name).split(/\s+/).filter(Boolean).slice(0, 2).map((part) => part[0]).join("").toUpperCase() || "U";

  return <SharedAppShell apiAvailable={apiAvailable} session={session} signOut={signOut} sidebar={<OrganizationRail organizations={organizations} />}>
    <section className="shared-page-content shared-profile-page" aria-busy={isLoading}>
      <div className="shared-detail-heading"><div><p className="shared-eyebrow">Account</p><h1>Your profile</h1><p>Manage your identity and review your activity across shared workspaces.</p></div><Button onClick={() => navigate("/workspaces")} type="button" variant="secondary"><ArrowLeft size={16} /> All workspaces</Button></div>
      {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
      {notice ? <p className="shared-profile-notice" role="status">{notice}</p> : null}
      <div className="shared-profile-layout">
        <section className="shared-profile-identity"><div className="shared-profile-avatar" aria-hidden="true">{initials}</div><div><h2>{profile?.user.display_name ?? session.user.display_name}</h2><p>{profile?.user.email ?? session.user.email ?? "No email address"}</p></div><dl><div><dt>Last connected</dt><dd>{formatProfileTime(profile?.last_connected_at)}</dd></div><div><dt>Member since</dt><dd>{formatProfileTime(profile?.created_at)}</dd></div><div><dt>Shared workspaces</dt><dd>{profile?.workspace_count ?? "—"}</dd></div><div><dt>Recorded actions</dt><dd>{profile?.recent_activity_count ?? "—"}</dd></div></dl></section>
        <ContributionCalendar items={profile?.activity_by_day ?? []} title="Your activity" total={profile?.recent_activity_count} />
        <section className="shared-profile-assigned"><div className="shared-panel-heading"><div><Checklist size={18} /><h2>Assigned to you</h2></div><span>{assignedTasks.length} active</span></div>{assignedTasks.length ? <div className="shared-profile-task-list">{assignedTasks.map((task) => <Button key={task.id} onClick={() => navigate(`/workspaces/${encodeURIComponent(task.workspace_id)}/tasks`)} type="button" variant="secondary"><span><strong>{task.title}</strong><small>{task.status.replace(/_/g, " ")} · {task.priority} priority{task.due_at ? ` · due ${formatTaskDueDate(task.due_at)}` : ""}</small></span><ChevronRight size={16} /></Button>)}</div> : <p className="shared-muted-copy">No active tasks are assigned to you.</p>}</section>
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><UserCircle size={18} /><h2>Personal details</h2></div></div><form onSubmit={saveProfile}><label>Display name<Input autoComplete="name" onChange={(event) => setDisplayName(event.target.value)} required value={displayName} /></label><label>Email<Input disabled value={profile?.user.email ?? session.user.email ?? ""} /></label><Button disabled={isSubmitting} type="submit" variant="main">{isSubmitting ? <Loader className="spin" size={16} /> : <Pencil size={16} />} Save details</Button></form></section>
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><Key size={18} /><h2>Password</h2></div></div><form onSubmit={changePassword}><label>Current password<Input autoComplete="current-password" minLength={12} onChange={(event) => setCurrentPassword(event.target.value)} required type="password" value={currentPassword} /></label><label>New password<Input autoComplete="new-password" minLength={12} onChange={(event) => setNewPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={newPassword} /></label><label>Confirm new password<Input autoComplete="new-password" minLength={12} onChange={(event) => setConfirmPassword(event.target.value)} required type="password" value={confirmPassword} /></label><Button disabled={isSubmitting} type="submit" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Key size={16} />} Change password</Button></form></section>
      </div>
    </section>
  </SharedAppShell>;
}

function SharedNotifications({
  accessToken,
  apiAvailable,
  organizations,
  session,
  signOut,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
}) {
  const [notifications, setNotifications] = useState<SharedNotification[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isMutating, setIsMutating] = useState(false);

  async function load() {
    setIsLoading(true); setError(null);
    try { setNotifications(await listSharedNotifications(accessToken)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsLoading(false); }
  }

  useEffect(() => { void load(); }, [accessToken]);

  async function openNotification(notification: SharedNotification) {
    setError(null);
    try {
      if (!notification.read_at) {
        const updated = await markSharedNotificationRead(accessToken, notification.id);
        setNotifications((current) => current.map((entry) => entry.id === updated.id ? updated : entry));
      }
      navigate(notification.href);
    } catch (requestError) { setError(apiMessage(requestError)); }
  }

  async function markAllRead() {
    setIsMutating(true); setError(null);
    try {
      await markAllSharedNotificationsRead(accessToken);
      setNotifications((current) => current.map((entry) => entry.read_at ? entry : { ...entry, read_at: new Date().toISOString() }));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  const unreadCount = notifications.filter((notification) => !notification.read_at).length;
  return <SharedAppShell apiAvailable={apiAvailable} session={session} signOut={signOut} sidebar={<OrganizationRail organizations={organizations} />}>
    <section className="shared-page-content shared-notifications-page" aria-busy={isLoading}>
      <div className="shared-detail-heading"><div><p className="shared-eyebrow">Inbox</p><h1>Notifications</h1><p>Task assignments and direct evidence mentions appear here. Mention a teammate with their email, for example <code>@person@example.com</code>.</p></div><div className="shared-detail-actions"><Button disabled={isLoading} onClick={() => void load()} type="button" variant="secondary"><Refresh size={16} /> Refresh</Button><Button disabled={isMutating || unreadCount === 0} onClick={() => void markAllRead()} type="button" variant="secondary">Mark all read</Button></div></div>
      {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
      <section className="shared-notification-list" aria-label="Notifications">{notifications.length ? notifications.map((notification) => <article className={notification.read_at ? "" : "is-unread"} key={notification.id}><Button onClick={() => void openNotification(notification)} type="button" variant="secondary"><span className="shared-notification-icon"><Bell size={17} /></span><span><strong>{notification.title}</strong><small>{notification.body}</small><time dateTime={notification.created_at}>{formatActivityTime(notification.created_at)}</time></span><ChevronRight size={17} /></Button></article>) : <div className="shared-empty-state"><Bell size={25} /><strong>Nothing new</strong><span>Assignments and evidence mentions will appear here when your team needs your attention.</span></div>}</section>
    </section>
  </SharedAppShell>;
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
  const [theme, setTheme] = useState<Theme>(initialTheme);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  return (
    <main className="shared-home">
      <header className="shared-home-header">
        <div className="shared-brand"><span className="shared-brand-glyph"><Layers size={19} /></span><strong>RepoMemo</strong><span className="shared-mode-tag">Shared</span></div>
        <div className="shared-user-menu"><Button aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} mode`} aria-pressed={theme === "dark"} className="shared-theme-toggle" onClick={() => setTheme((current) => current === "dark" ? "light" : "dark")} title={`Switch to ${theme === "dark" ? "light" : "dark"} mode`} type="button" variant="secondary">{theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}<span>{theme === "dark" ? "Light" : "Dark"}</span></Button><Button className="shared-notifications-link" onClick={() => navigate("/notifications")} type="button" variant="secondary"><Bell size={16} /> Notifications</Button><Button className="shared-profile-link" onClick={() => navigate("/profile")} type="button" variant="secondary"><UserCircle size={16} /> {session.user.display_name}</Button><Button className="shared-user-signout" onClick={signOut} type="button" variant="secondary"><Logout size={16} /> Sign out</Button></div>
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
        <Button aria-current={activeSection === "tasks" ? "page" : undefined} className={activeSection === "tasks" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("tasks")} type="button" variant="secondary"><Checklist size={16} /> Tasks</Button>
        <Button aria-current={activeSection === "people" ? "page" : undefined} className={activeSection === "people" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("people")} type="button" variant="secondary"><Users size={16} /> People</Button>
        <Button aria-current={activeSection === "activity" ? "page" : undefined} className={activeSection === "activity" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("activity")} type="button" variant="secondary"><Timeline size={16} /> Activity</Button>
        {workspace.role === "owner" || workspace.role === "admin" ? <Button aria-current={activeSection === "settings" ? "page" : undefined} className={activeSection === "settings" ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.("settings")} type="button" variant="secondary"><Settings size={16} /> Settings</Button> : null}
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
  const [workspaceMetrics, setWorkspaceMetrics] = useState<WorkspaceMetrics | null>(null);
  const [artifacts, setArtifacts] = useState<ArtifactSummary[]>([]);
  const [artifactResults, setArtifactResults] = useState<ArtifactSummary[] | null>(null);
  const [artifactQuery, setArtifactQuery] = useState("");
  const [artifactType, setArtifactType] = useState<ArtifactType | "all">("all");
  const [artifactSourceId, setArtifactSourceId] = useState("all");
  const [artifactIndexStatus, setArtifactIndexStatus] = useState<"all" | "indexed" | "not_indexed">("all");
  const [artifactViewMode, setArtifactViewMode] = useState<ArtifactViewMode>("grid");
  const [memoryCards, setMemoryCards] = useState<MemoryCardSummary[]>([]);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [query, setQuery] = useState("");
  const [retrievalFacets, setRetrievalFacets] = useState<RetrievalFacets | null>(null);
  const [retrievalArtifactType, setRetrievalArtifactType] = useState<ArtifactType | "all">("all");
  const [retrievalLanguage, setRetrievalLanguage] = useState("all");
  const [retrievalSourceId, setRetrievalSourceId] = useState("all");
  const [retrievalLimit, setRetrievalLimit] = useState("20");
  const [askQuestion, setAskQuestion] = useState("");
  const [askAnswer, setAskAnswer] = useState<AskAnswer | null>(null);
  const [memoryQuery, setMemoryQuery] = useState("");
  const [memoryResults, setMemoryResults] = useState<MemoryCardSummary[] | null>(null);
  const [noteTitle, setNoteTitle] = useState("");
  const [noteContent, setNoteContent] = useState("");
  const [uploadFile, setUploadFile] = useState<File | null>(null);
  const [memoryTitle, setMemoryTitle] = useState("");
  const [memoryBody, setMemoryBody] = useState("");
  const [memoryArtifactId, setMemoryArtifactId] = useState("");
  const [members, setMembers] = useState<WorkspaceMember[]>([]);
  const [activity, setActivity] = useState<WorkspaceActivityEvent[]>([]);
  const [activityCalendar, setActivityCalendar] = useState<WorkspaceActivityCalendar | null>(null);
  const [tasks, setTasks] = useState<CollaborationTask[]>([]);
  const [taskQuery, setTaskQuery] = useState("");
  const [taskAssigneeFilter, setTaskAssigneeFilter] = useState("all");
  const [taskTitle, setTaskTitle] = useState("");
  const [taskDescription, setTaskDescription] = useState("");
  const [taskPriority, setTaskPriority] = useState<CollaborationTaskPriority>("medium");
  const [taskAssigneeId, setTaskAssigneeId] = useState("");
  const [taskArtifactId, setTaskArtifactId] = useState("");
  const [taskDueDate, setTaskDueDate] = useState("");
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
  const [providerTest, setProviderTest] = useState<ProviderTestResult | null>(null);
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
  const isTasksView = section === "tasks";
  const isPeopleView = section === "people";
  const isActivityView = section === "activity";
  const isSettingsView = section === "settings";
  const displayedArtifacts = artifactResults ?? artifacts;
  const artifactSources = Array.from(new Map(artifacts.map((artifact) => [artifact.source_id, artifact.source_name])).entries());
  const visibleTasks = tasks.filter((task) => {
    const normalizedQuery = taskQuery.trim().toLowerCase();
    const matchesQuery = !normalizedQuery || `${task.title} ${task.description}`.toLowerCase().includes(normalizedQuery);
    const matchesAssignee = taskAssigneeFilter === "all"
      || (taskAssigneeFilter === "unassigned" ? !task.assignee : task.assignee?.id === taskAssigneeFilter);
    return matchesQuery && matchesAssignee;
  });

  async function load() {
    setIsLoading(true);
    setError(null);
    try {
      const [nextOverview, nextWorkspaceMetrics, nextArtifacts, nextMemory, nextMembers, nextCapabilities, nextRetrievalFacets] = await Promise.all([
        getSharedWorkspaceOverview(accessToken, workspace.workspace.id),
        getSharedWorkspaceMetrics(accessToken, workspace.workspace.id),
        listSharedArtifacts(accessToken, workspace.workspace.id),
        listSharedMemoryCards(accessToken, workspace.workspace.id),
        listSharedWorkspaceMembers(accessToken, workspace.workspace.id),
        getSharedWorkspaceCapabilities(accessToken, workspace.workspace.id),
        getSharedRetrievalFacets(accessToken, workspace.workspace.id),
      ]);
      setOverview(nextOverview);
      setWorkspaceMetrics(nextWorkspaceMetrics);
      setArtifacts(nextArtifacts);
      setArtifactResults(null);
      setMemoryCards(nextMemory);
      setMembers(nextMembers);
      setCapabilities(nextCapabilities);
      setRetrievalFacets(nextRetrievalFacets);
      if (nextCapabilities.can_manage_members) {
        setAiProviders(await listSharedWorkspaceAiProviders(accessToken, workspace.workspace.id));
      } else {
        setAiProviders([]);
      }
      if (isActivityView) {
        const [nextActivity, nextActivityCalendar] = await Promise.all([
          listSharedWorkspaceActivity(accessToken, workspace.workspace.id),
          getSharedWorkspaceActivityCalendar(accessToken, workspace.workspace.id),
        ]);
        setActivity(nextActivity);
        setActivityCalendar(nextActivityCalendar);
      }
      if (isTasksView) {
        setTasks(await listSharedCollaborationTasks(accessToken, workspace.workspace.id));
      }
    } catch (requestError) {
      setError(apiMessage(requestError));
    } finally {
      setIsLoading(false);
    }
  }

  useEffect(() => { void load(); }, [accessToken, isActivityView, isTasksView, workspace.workspace.id]);
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

  async function filterArtifacts(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      setArtifactResults(await querySharedArtifacts(accessToken, workspace.workspace.id, {
        query: artifactQuery,
        artifactTypes: artifactType === "all" ? [] : [artifactType],
        sourceIds: artifactSourceId === "all" ? [] : [artifactSourceId],
        indexed: artifactIndexStatus === "all" ? undefined : artifactIndexStatus === "indexed",
      }));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function clearArtifactFilters() {
    setArtifactQuery("");
    setArtifactType("all");
    setArtifactSourceId("all");
    setArtifactIndexStatus("all");
    setArtifactResults(null);
  }

  async function askEvidence(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!askQuestion.trim()) return;
    setIsSubmitting(true); setError(null);
    try { setAskAnswer(await askSharedWorkspace(accessToken, workspace.workspace.id, askQuestion)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function runMemorySearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextQuery = memoryQuery.trim();
    if (!nextQuery) { setMemoryResults(null); return; }
    setIsSubmitting(true); setError(null);
    try { setMemoryResults(await searchSharedMemoryCards(accessToken, workspace.workspace.id, nextQuery)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
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
      setProviderTest(null);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function testAiProvider() {
    if (!providerId) {
      setError("Save the AI provider before testing its connection.");
      return;
    }
    setIsSubmitting(true); setError(null); setProviderTest(null);
    try { setProviderTest(await testSharedWorkspaceAiProvider(accessToken, workspace.workspace.id, providerId)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
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
          <div><p className="shared-eyebrow">{isEvidenceView ? "Evidence" : isRetrievalView ? "Retrieval" : isMemoryView ? "Team memory" : isTasksView ? "Team follow-up" : isPeopleView ? "Workspace access" : isActivityView ? "Workspace history" : isSettingsView ? "Administrative control" : `Server workspace · ${workspace.role}`}</p><h1>{isEvidenceView ? "Evidence ledger" : isRetrievalView ? "Retrieve evidence" : isMemoryView ? "Durable team memory" : isTasksView ? "Tasks" : isPeopleView ? "People" : isActivityView ? "Activity" : isSettingsView ? "Workspace settings" : workspace.workspace.name}</h1><p>{isEvidenceView ? "Store notes and files in the workspace, then index them for retrieval." : isRetrievalView ? "Search indexed workspace evidence and inspect the source context behind every result." : isMemoryView ? "Capture concise facts and decisions that should outlive the current investigation." : isTasksView ? "Turn evidence and decisions into assigned, trackable action items for the team." : isPeopleView ? "See who can access this workspace and understand each person’s role." : isActivityView ? "A durable record of shared workspace changes, including evidence, memory, indexing, and membership updates." : isSettingsView ? "Administrators control workspace access and AI integrations here. Owner-only actions remain clearly marked." : "Artifacts, search results, and durable team memory are all retrieved through the protected shared API."}</p></div>
          <div className="shared-detail-actions"><Button className="shared-back-button" onClick={onBack} type="button" variant="secondary"><ArrowLeft size={16} /> All workspaces</Button><Button disabled={isLoading} onClick={() => void load()} type="button" variant="secondary"><Refresh size={16} /> Refresh</Button>{canWrite && isEvidenceView ? <Button disabled={isSubmitting || isLoading} onClick={() => void runIndex()} type="button" variant="main">{isSubmitting ? <Loader className="spin" size={16} /> : <Layers size={16} />} Index evidence</Button> : null}</div>
        </div>
        {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
        {section === "overview" ? <><div className="shared-evidence-summary">
          <span><strong>{overview?.artifact_count ?? "—"}</strong> artifacts</span><span><strong>{overview?.chunk_count ?? "—"}</strong> indexed chunks</span><span><strong>{overview?.memory_card_count ?? "—"}</strong> memory cards</span>
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
          {aiOverview?.summary_markdown ? <div className="shared-ai-overview-body">{aiOverview.summary_markdown}</div> : <p className="shared-muted-copy">Generate a concise project briefing from indexed evidence. RepoMemo only uses an enabled provider configured for this workspace and returns the source citations with the result.</p>}
          {aiOverview?.warnings.length ? <div className="shared-ai-overview-warning">{aiOverview.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}
          {aiOverview?.citations.length ? <div className="shared-ai-citations"><strong>Evidence used</strong>{aiOverview.citations.map((citation) => <span key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}`}>{citation.title} · {citation.path}{citation.start_line ? ` · line ${citation.start_line}` : ""}</span>)}</div> : null}
          <Button disabled={isSubmitting || isLoading || !capabilities?.can_generate_ai_overview} onClick={() => void generateAiOverview()} type="button" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Brain size={16} />}{aiOverview?.summary_markdown ? "Refresh AI overview" : "Generate AI overview"}</Button>
        </section> : null}
        {section !== "overview" ? <div className={`shared-detail-grid${isEvidenceView ? " evidence-only" : isRetrievalView ? " retrieval-only" : isMemoryView ? " memory-only" : isTasksView ? " tasks-only" : isPeopleView ? " people-only" : isActivityView ? " activity-only" : isSettingsView ? " settings-only" : ""}`}>
          {isEvidenceView ? <section className="shared-detail-panel">
            <div className="shared-panel-heading"><div><FileText size={18} /><h2>Evidence ledger</h2></div><span>{displayedArtifacts.length} of {artifacts.length} files</span></div>
            <div className="shared-artifact-browser">
              <form className="shared-artifact-filters" onSubmit={filterArtifacts}>
                <Input aria-label="Search evidence files" onChange={(event) => setArtifactQuery(event.target.value)} placeholder="Search file name or path" value={artifactQuery} />
                <Dropdown aria-label="Filter evidence by file type" onValueChange={(value) => setArtifactType(value as ArtifactType | "all")} options={[{ label: "All file types", value: "all" }, ...Array.from(new Set(artifacts.map((artifact) => artifact.artifact_type))).map((type) => ({ label: artifactTypeLabel(type), value: type }))]} value={artifactType} />
                <Dropdown aria-label="Filter evidence by source" onValueChange={setArtifactSourceId} options={[{ label: "All sources", value: "all" }, ...artifactSources.map(([id, name]) => ({ label: name, value: id }))]} value={artifactSourceId} />
                <Dropdown aria-label="Filter evidence by indexing status" onValueChange={(value) => setArtifactIndexStatus(value as "all" | "indexed" | "not_indexed")} options={[{ label: "Any indexing status", value: "all" }, { label: "Indexed", value: "indexed" }, { label: "Not indexed", value: "not_indexed" }]} value={artifactIndexStatus} />
                <Button disabled={isSubmitting} type="submit" variant="secondary"><Filter size={16} /> Apply</Button>
                {artifactResults ? <Button disabled={isSubmitting} onClick={clearArtifactFilters} type="button" variant="secondary">Clear</Button> : null}
              </form>
              <div className="shared-artifact-browser-meta"><span>{artifactResults ? "Filtered server results" : "All workspace files"}</span><div className="shared-artifact-view-switch" role="group" aria-label="Evidence view"><Button aria-label="Grid view" aria-pressed={artifactViewMode === "grid"} className={artifactViewMode === "grid" ? "active" : ""} onClick={() => setArtifactViewMode("grid")} type="button" variant="secondary"><Grid size={16} /></Button><Button aria-label="List view" aria-pressed={artifactViewMode === "list"} className={artifactViewMode === "list" ? "active" : ""} onClick={() => setArtifactViewMode("list")} type="button" variant="secondary"><List size={16} /></Button></div></div>
              {displayedArtifacts.length ? <div className={`shared-artifact-manager ${artifactViewMode}`}>{displayedArtifacts.map((artifact) => <article key={artifact.id}><Button className="shared-artifact-entry" onClick={() => onOpenArtifact(artifact.id)} type="button" variant="secondary"><span className="shared-artifact-file-icon"><FileText size={20} /></span><span className="shared-artifact-entry-copy"><strong>{artifact.title}</strong><span>{artifact.path}</span></span><span className="shared-artifact-entry-meta"><span>{artifactTypeLabel(artifact.artifact_type)}</span><span>{artifact.language ?? "Unspecified"}</span><span>{formatFileSize(artifact.size_bytes)}</span><span className={artifact.indexed_at ? "indexed" : "pending"}>{artifact.indexed_at ? "Indexed" : "Not indexed"}</span></span></Button></article>)}</div> : <div className="shared-empty-state"><FileText size={25} /><strong>{artifacts.length ? "No files match these filters" : "No shared evidence yet"}</strong><span>{artifacts.length ? "Clear or adjust the filters to see other workspace files." : "Add a pasted note below, then index it when you are ready to search."}</span></div>}
            </div>
            {canWrite ? <><form className="shared-note-form" onSubmit={addNote}><h3>Add shared note</h3><Input onChange={(event) => setNoteTitle(event.target.value)} placeholder="Decision or implementation note" required value={noteTitle} /><Textarea onChange={(event) => setNoteContent(event.target.value)} placeholder="Paste Markdown, code context, or a meeting note…" required value={noteContent} /><Button disabled={isSubmitting} type="submit" variant="main"><Plus size={16} /> Store evidence</Button></form><form className="shared-upload-form" onSubmit={submitUpload}><div><strong>Upload a file</strong><span>Markdown, text, code, or supported image · up to 10 MiB</span></div><label className="shared-upload-picker"><input accept=".md,.mdx,.txt,.rs,.ts,.tsx,.js,.jsx,.py,.json,.toml,.yaml,.yml,.sql,.html,.css,.sh,.ps1,.png,.jpg,.jpeg,.gif,.webp,.svg,.bmp" aria-label="Upload a shared artifact" className="shared-upload-native-input" onChange={(event) => setUploadFile(event.target.files?.[0] ?? null)} required type="file" /><span className="shared-upload-picker-icon"><Upload size={18} /></span><span className="shared-upload-picker-copy"><strong>{uploadFile?.name ?? "Choose a shared file"}</strong><span>{uploadFile ? `${formatFileSize(uploadFile.size)} · ready to upload` : "Markdown, text, code, or a supported image"}</span></span><span className="shared-upload-picker-action">Browse</span></label><Button disabled={isSubmitting || !uploadFile} type="submit" variant="secondary"><Upload size={16} /> Upload</Button></form></> : <p className="shared-readonly-note"><Shield size={15} /> Your viewer membership can inspect shared evidence but cannot change it.</p>}
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
            <div>{results.length ? <div className="shared-search-results"><p className="shared-search-result-count">{results.length} matching evidence {results.length === 1 ? "result" : "results"}</p>{results.map((result) => <article key={result.chunk_id}><Button className="shared-search-result" onClick={() => onOpenArtifact(result.artifact_id)} type="button" variant="secondary"><span className="shared-search-result-heading"><strong>{result.title}</strong><span>{artifactTypeLabel(result.artifact_type)} · {result.language ?? "Unspecified"} · {result.source_name}</span></span><p>{result.snippet}</p><span className="shared-search-result-path">{result.path}{result.start_line ? ` · line ${result.start_line}` : ""}</span></Button></article>)}</div> : <p className="shared-muted-copy">Index one or more artifacts, then search the evidence base from here. Use filters to narrow large workspaces.</p>}
              <section className="shared-ask-evidence" aria-live="polite"><div><Brain size={18} /><h3>Ask your evidence</h3></div><p>Get a citation-backed answer from the indexed workspace context.</p><form onSubmit={askEvidence}><Textarea onChange={(event) => setAskQuestion(event.target.value)} placeholder="What do we know about the current implementation?" required value={askQuestion} /><Button disabled={isSubmitting} type="submit" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Brain size={16} />} Ask</Button></form>{askAnswer ? <div className="shared-ask-answer"><div className="shared-ai-overview-body">{askAnswer.answer_markdown}</div>{askAnswer.warnings.length ? <div className="shared-ai-overview-warning">{askAnswer.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}{askAnswer.citations.length ? <div className="shared-ai-citations"><strong>Evidence used</strong>{askAnswer.citations.map((citation) => <span key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}`}>{citation.title} · {citation.path}{citation.start_line ? ` · line ${citation.start_line}` : ""}</span>)}</div> : null}</div> : null}</section>
            </div></> : null}
            {isMemoryView ? <div className="shared-memory-section"><div className="shared-panel-heading"><div><Shield size={18} /><h2>Team memory</h2></div><span>{memoryResults?.length ?? memoryCards.length}</span></div><form className="shared-search-form shared-memory-search" onSubmit={runMemorySearch}><Input onChange={(event) => setMemoryQuery(event.target.value)} placeholder="Search team memory" value={memoryQuery} /><Button disabled={isSubmitting} type="submit" variant="secondary">Search</Button></form>{(memoryResults ?? memoryCards).length ? <div className="shared-memory-list">{(memoryResults ?? memoryCards).map((card) => <article key={card.id}><Button className="shared-record-link" onClick={() => onOpenMemoryCard(card.id)} type="button" variant="secondary"><strong>{card.title}</strong><span>{card.body_excerpt}</span></Button></article>)}</div> : <p className="shared-muted-copy">{memoryResults ? "No memory cards match that search." : "No durable memory cards yet."}</p>}{canWrite ? <form className="shared-memory-form" onSubmit={addMemory}><Input onChange={(event) => setMemoryTitle(event.target.value)} placeholder="Memory title" required value={memoryTitle} /><Textarea onChange={(event) => setMemoryBody(event.target.value)} placeholder="A concise durable fact…" required value={memoryBody} /><label>Evidence link<Dropdown aria-label="Evidence link" onValueChange={(value) => setMemoryArtifactId(value === "__none__" ? "" : value)} options={[{ label: "No direct artifact link", value: "__none__" }, ...artifacts.map((artifact) => ({ label: artifact.title, value: artifact.id }))]} value={memoryArtifactId || "__none__"} /></label><Button disabled={isSubmitting} type="submit" variant="secondary">Save memory</Button></form> : null}</div> : null}
            {isTasksView ? <div className="shared-task-workspace">
              <div className="shared-panel-heading"><div><Checklist size={18} /><h2>Team action board</h2></div><span>{tasks.length} tasks</span></div>
              <div className="shared-task-toolbar"><Input aria-label="Search tasks" onChange={(event) => setTaskQuery(event.target.value)} placeholder="Search action items" value={taskQuery} /><Dropdown aria-label="Filter tasks by assignee" onValueChange={setTaskAssigneeFilter} options={[{ label: "Everyone", value: "all" }, { label: "Unassigned", value: "unassigned" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={taskAssigneeFilter} /></div>
              {canWrite ? <form className="shared-task-create" onSubmit={addTask}><div className="shared-task-create-heading"><div><Plus size={17} /><div><strong>Create an action item</strong><span>Assign work and tie it back to the evidence that prompted it.</span></div></div></div><Input aria-label="Task title" onChange={(event) => setTaskTitle(event.target.value)} placeholder="What needs to happen?" required value={taskTitle} /><Textarea aria-label="Task description" onChange={(event) => setTaskDescription(event.target.value)} placeholder="Context, acceptance criteria, or next steps" value={taskDescription} /><div className="shared-task-create-fields"><Dropdown aria-label="Task priority" onValueChange={(value) => setTaskPriority(value as CollaborationTaskPriority)} options={[{ label: "Low priority", value: "low" }, { label: "Medium priority", value: "medium" }, { label: "High priority", value: "high" }, { label: "Urgent", value: "urgent" }]} value={taskPriority} /><Dropdown aria-label="Task assignee" onValueChange={(value) => setTaskAssigneeId(value === "__unassigned__" ? "" : value)} options={[{ label: "Unassigned", value: "__unassigned__" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={taskAssigneeId || "__unassigned__"} /><Dropdown aria-label="Related evidence" onValueChange={(value) => setTaskArtifactId(value === "__none__" ? "" : value)} options={[{ label: "No evidence link", value: "__none__" }, ...artifacts.map((artifact) => ({ label: artifact.title, value: artifact.id }))]} value={taskArtifactId || "__none__"} /><Input aria-label="Task due date" onChange={(event) => setTaskDueDate(event.target.value)} type="date" value={taskDueDate} /></div><Button disabled={isSubmitting} type="submit" variant="main">Create task</Button></form> : null}
              {visibleTasks.length ? <div className="shared-task-board">{TASK_COLUMNS.map((column) => { const columnTasks = visibleTasks.filter((task) => task.status === column.status); return <section className="shared-task-column" key={column.status}><div className="shared-task-column-heading"><h3>{column.label}</h3><span>{columnTasks.length}</span></div><div className="shared-task-column-list">{columnTasks.length ? columnTasks.map((task) => <article className={`shared-task-card priority-${task.priority}${isTaskOverdue(task) ? " overdue" : ""}`} key={task.id}><div className="shared-task-card-heading"><span>{task.priority}</span>{task.due_at ? <time dateTime={task.due_at}>{formatTaskDueDate(task.due_at)}</time> : null}</div><h4>{task.title}</h4>{task.description ? <p>{task.description}</p> : null}<div className="shared-task-context"><span>{task.assignee?.display_name ?? "Unassigned"}</span>{task.artifact_id ? <Button onClick={() => onOpenArtifact(task.artifact_id!)} type="button" variant="secondary"><FileText size={14} /> Evidence</Button> : null}</div>{canWrite ? <div className="shared-task-controls"><Dropdown aria-label={`Status for ${task.title}`} onValueChange={(value) => void reviseTask(task, { status: value as CollaborationTaskStatus })} options={TASK_COLUMNS.map((option) => ({ label: option.label, value: option.status }))} value={task.status} /><Dropdown aria-label={`Assignee for ${task.title}`} onValueChange={(value) => void reviseTask(task, { assigneeUserId: value === "__unassigned__" ? null : value })} options={[{ label: "Unassigned", value: "__unassigned__" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={task.assignee?.id ?? "__unassigned__"} /><Dropdown aria-label={`Priority for ${task.title}`} onValueChange={(value) => void reviseTask(task, { priority: value as CollaborationTaskPriority })} options={[{ label: "Low", value: "low" }, { label: "Medium", value: "medium" }, { label: "High", value: "high" }, { label: "Urgent", value: "urgent" }]} value={task.priority} />{task.created_by.id === session.user.id || workspace.role === "owner" || workspace.role === "admin" ? <Button aria-label={`Delete ${task.title}`} disabled={isSubmitting} onClick={() => void removeTask(task)} type="button" variant="secondary"><Trash size={15} /></Button> : null}</div> : null}</article>) : <p>No tasks here.</p>}</div></section>; })}</div> : <div className="shared-empty-state"><Checklist size={25} /><strong>No action items match</strong><span>Create a task or adjust the current filters.</span></div>}
            </div> : null}
            {isPeopleView ? <div className="shared-members-section">
              <div className="shared-panel-heading"><div><Users size={18} /><h2>Workspace team</h2></div><span>{members.length}</span></div>
              <p className="shared-muted-copy">{canManageMembers ? "Workspace access is administered from Settings." : "Your workspace role does not allow membership changes."}</p>
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
                </article>)}
              </div>
            </div> : null}
            {isActivityView ? <div className="shared-activity-section">
              <ContributionCalendar items={activityCalendar?.activity_by_day ?? []} title="Workspace activity" total={activityCalendar?.total_activity_count} />
              <div className="shared-panel-heading"><div><Timeline size={18} /><h2>Recent activity</h2></div><span>{activity.length}</span></div>
              {activity.length ? <div className="shared-activity-list">{activity.map((event) => <article key={event.id}><div><strong>{event.summary}</strong><span>{event.actor?.display_name ?? "System"} · {event.action.replace(/_/g, " ")}</span></div><time dateTime={event.created_at}>{formatActivityTime(event.created_at)}</time></article>)}</div> : <div className="shared-empty-state"><Timeline size={25} /><strong>No recorded activity yet</strong><span>New workspace changes will appear here.</span></div>}
            </div> : null}
          </aside> : null}
          {isSettingsView ? <section className="shared-settings-section">
            {canManageMembers ? <>
              <div className="shared-settings-policy"><Settings size={18} /><div><strong>Administrative workspace controls</strong><span>Changes are enforced by the shared API. Administrators can manage people and integrations; owner-only workspace actions are explicitly marked.</span></div></div>
              <section className="shared-settings-group">
                <div className="shared-panel-heading"><div><Users size={18} /><h2>People and access</h2></div><span>{members.length} people</span></div>
                <p className="shared-muted-copy">Add a registered RepoMemo user or update their role. Administrators cannot create, edit, or remove other administrators; owners retain the final authority.</p>
                <div className="shared-member-list">{members.map((member) => <article className="shared-member-row" key={member.user.id}><div><strong>{member.user.display_name}</strong><span>{member.user.email ?? "No email"}</span></div><span className="shared-member-role">{member.role}</span>{member.role !== "owner" ? <Button className="shared-member-remove" disabled={isSubmitting} onClick={() => void removeMember(member.user.id)} type="button" variant="secondary">Remove</Button> : null}</article>)}</div>
                <form className="shared-member-form" onSubmit={saveMember}><Input aria-label="Member email" onChange={(event) => setMemberEmail(event.target.value)} placeholder="person@example.com" required type="email" value={memberEmail} /><Dropdown aria-label="Member role" onValueChange={(value) => setMemberRole(value as WorkspaceRole)} options={[{ label: "Viewer", value: "viewer" }, { label: "Member", value: "member" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} value={memberRole} /><Button disabled={isSubmitting} type="submit" variant="secondary">Add / update</Button></form>
              </section>
              <section className="shared-settings-group">
                <div className="shared-panel-heading"><div><Brain size={18} /><h2>AI integration</h2></div><span>{aiProviders.some((provider) => provider.enabled) ? "configured" : "not configured"}</span></div>
                <p className="shared-muted-copy">Configure one explicit provider for citation-backed workspace overviews. Credentials are stored on the protected server and are never returned to the browser.</p>
                <form className="shared-ai-provider-form" onSubmit={saveAiProvider}>
                  <label>Provider<Dropdown aria-label="AI provider" onValueChange={(value) => selectProviderType(value as "ollama" | "openrouter")} options={[{ label: "Ollama (local)", value: "ollama" }, { label: "OpenRouter (cloud)", value: "openrouter" }]} value={providerType} /></label>
                  <label>Provider name<Input onChange={(event) => setProviderName(event.target.value)} required value={providerName} /></label>
                  <label>Base URL<Input onChange={(event) => setProviderBaseUrl(event.target.value)} placeholder={providerType === "ollama" ? "http://127.0.0.1:11434" : "https://openrouter.ai/api/v1"} value={providerBaseUrl} /></label>
                  <label>Chat model<Input onChange={(event) => setProviderModel(event.target.value)} placeholder={providerType === "ollama" ? "llama3.2" : "openai/gpt-4o-mini"} required value={providerModel} /></label>
                  {providerType === "openrouter" ? <><label>API key<Input autoComplete="off" onChange={(event) => setProviderApiKey(event.target.value)} placeholder={providerId ? "Leave blank to keep the saved key" : "Required to enable cloud AI"} type="password" value={providerApiKey} /></label><label className="shared-ai-provider-toggle"><input checked={cloudContentAcknowledged} onChange={(event) => setCloudContentAcknowledged(event.target.checked)} type="checkbox" /> I understand that generating an overview sends cited workspace excerpts to this cloud provider.</label></> : null}
                  <div className="shared-ai-provider-actions"><Button disabled={isSubmitting} type="submit" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Brain size={16} />} Save AI provider</Button><Button disabled={isSubmitting || !providerId} onClick={() => void testAiProvider()} type="button" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Refresh size={16} />} Test connection</Button></div>
                  {providerTest ? <p className={`shared-provider-test ${providerTest.success ? "success" : "failure"}`} role="status">{providerTest.message}</p> : null}
                </form>
              </section>
              {canManageWorkspace ? <section className="shared-settings-group">
                <div className="shared-panel-heading"><div><Settings size={18} /><h2>Workspace ownership</h2></div><span>owner only</span></div>
                <p className="shared-muted-copy">Rename this workspace or remove it permanently. Deletion removes its shared evidence, memory, and memberships.</p>
                <form className="shared-workspace-rename" onSubmit={renameWorkspace}><Input aria-label="Workspace name" onChange={(event) => setWorkspaceName(event.target.value)} required value={workspaceName} /><Button disabled={isSubmitting} type="submit" variant="secondary"><Pencil size={16} /> Rename</Button></form>
                <Button className="shared-danger-action" disabled={isSubmitting} onClick={() => void removeWorkspace()} type="button" variant="secondary"><Trash size={16} /> Delete workspace</Button>
              </section> : null}
            </> : <div className="shared-empty-state"><Settings size={25} /><strong>Administrative access required</strong><span>Only workspace administrators and the owner can open settings. Your current role can still browse shared evidence, memory, and activity.</span></div>}
          </section> : null}
        </div> : null}
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
  const [comments, setComments] = useState<ArtifactComment[]>([]);
  const [lifecycle, setLifecycle] = useState<ArtifactLifecycle | null>(null);
  const [lifecycleEvents, setLifecycleEvents] = useState<ArtifactLifecycleEvent[]>([]);
  const [workspaceMembers, setWorkspaceMembers] = useState<WorkspaceMember[]>([]);
  const [workspaceArtifacts, setWorkspaceArtifacts] = useState<ArtifactSummary[]>([]);
  const [commentBody, setCommentBody] = useState("");
  const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
  const [editingCommentBody, setEditingCommentBody] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isIndexing, setIsIndexing] = useState(false);
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

  async function load() {
    setIsLoading(true); setError(null);
    try {
      const [nextArtifact, nextComments, nextLifecycle, nextLifecycleEvents, nextMembers, nextArtifacts] = await Promise.all([
        getSharedArtifact(accessToken, artifactId),
        listSharedArtifactComments(accessToken, artifactId),
        getSharedArtifactLifecycle(accessToken, artifactId),
        listSharedArtifactLifecycleEvents(accessToken, artifactId),
        listSharedWorkspaceMembers(accessToken, workspace.workspace.id),
        listSharedArtifacts(accessToken, workspace.workspace.id),
      ]);
      setArtifact(nextArtifact); setComments(nextComments); setLifecycle(nextLifecycle); setLifecycleEvents(nextLifecycleEvents); setWorkspaceMembers(nextMembers); setWorkspaceArtifacts(nextArtifacts);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsLoading(false); }
  }

  useEffect(() => { void load(); }, [accessToken, artifactId]);
  useEffect(() => { setTitle(artifact?.summary.title ?? ""); }, [artifact?.summary.title]);
  useEffect(() => {
    setLifecycleStatus(lifecycle?.status ?? "active");
    setLifecycleOwnerId(lifecycle?.owner?.id ?? "");
    setLifecycleNote(lifecycle?.review_note ?? "");
    setSupersededByArtifactId(lifecycle?.superseded_by_artifact_id ?? "");
  }, [lifecycle]);

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
    <SharedRecordLayout activeSection="evidence" apiAvailable={apiAvailable} backLabel="Workspace" onBack={onBack} organization={organization} session={session} signOut={signOut} title={artifact?.summary.title ?? "Artifact"} subtitle={artifact?.summary.path ?? "Loading protected evidence…"} workspace={workspace}>
      <div className="shared-detail-actions"><Button disabled={isLoading} onClick={() => void load()} type="button" variant="secondary"><Refresh size={16} /> Refresh</Button>{canWrite ? <><Button disabled={isLoading || isMutating} onClick={() => setIsEditing((value) => !value)} type="button" variant="secondary"><Pencil size={16} /> {isEditing ? "Cancel edit" : "Edit"}</Button><Button className="shared-danger-action" disabled={isLoading || isMutating} onClick={() => void removeArtifact()} type="button" variant="secondary"><Trash size={16} /> Delete</Button><Button disabled={isLoading || isIndexing || isMutating} onClick={() => void indexArtifact()} type="button" variant="main">{isIndexing ? <Loader className="spin" size={16} /> : <Layers size={16} />} Index artifact</Button></> : null}</div>
      {error ? <p className="shared-form-error" role="alert">{error}</p> : null}
      <div className="shared-record-meta"><span>{artifact?.summary.artifact_type ?? "artifact"}</span><span>{artifact?.summary.language ?? "Unspecified language"}</span><span>{artifact?.summary.indexed_at ? "Indexed" : "Not indexed"}</span></div>
      {isEditing ? <form className="shared-record-edit-form" onSubmit={saveArtifact}><label>Evidence title<Input onChange={(event) => setTitle(event.target.value)} required value={title} /></label><Button disabled={isMutating} type="submit" variant="main">{isMutating ? <Loader className="spin" size={16} /> : <Pencil size={16} />} Save evidence</Button></form> : null}
      <section className="shared-record-panel shared-lifecycle-panel"><div className="shared-panel-heading"><div><Shield size={18} /><h2>Evidence lifecycle</h2></div><span className={`shared-lifecycle-status status-${lifecycle?.status ?? "active"}`}>{lifecycle?.status?.replace(/_/g, " ") ?? "Active"}</span></div>{isEditingLifecycle ? <form className="shared-lifecycle-form" onSubmit={saveLifecycle}><label>Status<Dropdown aria-label="Evidence lifecycle status" onValueChange={(value) => setLifecycleStatus(value as ArtifactLifecycleStatus)} options={[{ label: "Active", value: "active" }, { label: "Needs review", value: "needs_review" }, { label: "Verified", value: "verified" }, { label: "Outdated", value: "outdated" }, { label: "Superseded", value: "superseded" }]} value={lifecycleStatus} /></label><label>Review owner<Dropdown aria-label="Evidence review owner" onValueChange={(value) => setLifecycleOwnerId(value === "__unassigned__" ? "" : value)} options={[{ label: "Unassigned", value: "__unassigned__" }, ...workspaceMembers.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={lifecycleOwnerId || "__unassigned__"} /></label>{lifecycleStatus === "superseded" ? <label>Replacement evidence<Dropdown aria-label="Replacement evidence" onValueChange={(value) => setSupersededByArtifactId(value === "__none__" ? "" : value)} options={[{ label: "Choose replacement evidence", value: "__none__" }, ...workspaceArtifacts.filter((entry) => entry.id !== artifactId).map((entry) => ({ label: entry.title, value: entry.id }))]} value={supersededByArtifactId || "__none__"} /></label> : null}<label className="shared-lifecycle-note">Review note<Textarea onChange={(event) => setLifecycleNote(event.target.value)} placeholder="What should the team know about this evidence?" value={lifecycleNote} /></label><div><Button disabled={isMutating} type="submit" variant="main">{isMutating ? <Loader className="spin" size={16} /> : <Shield size={16} />} Save lifecycle</Button><Button disabled={isMutating} onClick={() => setIsEditingLifecycle(false)} type="button" variant="secondary">Cancel</Button></div></form> : <div className="shared-lifecycle-summary"><div><strong>{lifecycle?.owner ? `Owned by ${lifecycle.owner.display_name}` : "No review owner"}</strong><span>{lifecycle?.reviewed_at ? `Last updated ${formatActivityTime(lifecycle.reviewed_at)} by ${lifecycle.reviewed_by?.display_name ?? "a member"}` : "No lifecycle review recorded yet."}</span></div>{lifecycle?.review_note ? <p>{lifecycle.review_note}</p> : <p className="shared-muted-copy">Add a review note so the evidence can be trusted in context.</p>}{lifecycle?.superseded_by_artifact_id ? <span className="shared-lifecycle-replacement">Replaced by evidence {lifecycle.superseded_by_artifact_id.slice(0, 8)}</span> : null}{canWrite ? <Button onClick={() => setIsEditingLifecycle(true)} type="button" variant="secondary"><Pencil size={15} /> Update lifecycle</Button> : null}</div>}<div className="shared-lifecycle-history"><strong>History</strong>{lifecycleEvents.length ? lifecycleEvents.map((event) => <p key={event.id}><span>{event.detail}</span><time dateTime={event.created_at}>{event.actor?.display_name ?? "System"} · {formatActivityTime(event.created_at)}</time></p>) : <p className="shared-muted-copy">Lifecycle changes will be recorded here.</p>}</div></section>
      <section className="shared-record-panel"><h2>Stored content</h2>{artifact?.content_preview ? <pre className="shared-content-preview">{artifact.content_preview}</pre> : <p className="shared-muted-copy">This artifact has no text preview available.</p>}</section>
      <section className="shared-record-panel"><h2>Indexed evidence</h2>{artifact?.chunks.length ? <div className="shared-chunk-list">{artifact.chunks.map((chunk) => <article key={chunk.id}><span>{chunk.start_line ? `Lines ${chunk.start_line}${chunk.end_line && chunk.end_line !== chunk.start_line ? `–${chunk.end_line}` : ""}` : "Stored chunk"}</span><p>{chunk.text}</p></article>)}</div> : <p className="shared-muted-copy">Index this artifact to create retrievable evidence chunks.</p>}</section>
      <section className="shared-record-panel shared-discussion-panel"><div className="shared-panel-heading"><div><MessageCircle size={18} /><h2>Evidence discussion</h2></div><span>{comments.length} comments</span></div>{comments.length ? <div className="shared-comment-list">{comments.map((comment) => <article key={comment.id}><div className="shared-comment-author"><span aria-hidden="true">{comment.author.display_name.slice(0, 1).toUpperCase()}</span><div><strong>{comment.author.display_name}</strong><time dateTime={comment.created_at}>{formatActivityTime(comment.created_at)}{comment.updated_at !== comment.created_at ? " · edited" : ""}</time></div></div>{editingCommentId === comment.id ? <form onSubmit={saveComment}><Textarea aria-label="Edit comment" onChange={(event) => setEditingCommentBody(event.target.value)} required value={editingCommentBody} /><div><Button disabled={isMutating} type="submit" variant="main">Save comment</Button><Button onClick={() => setEditingCommentId(null)} type="button" variant="secondary">Cancel</Button></div></form> : <><p>{comment.body}</p>{comment.author.id === session.user.id || canModerateComments ? <div className="shared-comment-actions">{comment.author.id === session.user.id ? <Button onClick={() => { setEditingCommentId(comment.id); setEditingCommentBody(comment.body); }} type="button" variant="secondary"><Pencil size={14} /> Edit</Button> : null}<Button disabled={isMutating} onClick={() => void removeComment(comment)} type="button" variant="secondary"><Trash size={14} /> Delete</Button></div> : null}</>}</article>)}</div> : <p className="shared-muted-copy">No discussion yet. Add context, ask for a review, or record a decision beside the evidence.</p>}{canWrite ? <form className="shared-comment-form" onSubmit={addComment}><Textarea aria-label="New evidence comment" onChange={(event) => setCommentBody(event.target.value)} placeholder="Add context or mention @teammate@example.com…" required value={commentBody} /><Button disabled={isMutating} type="submit" variant="main"><MessageCircle size={16} /> Add comment</Button></form> : null}</section>
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
    <SharedAppShell apiAvailable={apiAvailable} session={session} signOut={signOut} sidebar={<WorkspaceRail activeSection={activeSection} onNavigate={(section) => navigate(`/workspaces/${encodeURIComponent(workspace.workspace.id)}/${section}`)} organization={organization} workspace={workspace} />}>
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
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function MetricBars({ emptyLabel, formatValue = (value: number) => value.toLocaleString(), items, summary, title }: { emptyLabel: string; formatValue?: (value: number) => string; items: WorkspaceMetricBreakdown[]; summary?: string; title: string }) {
  const visibleItems = items.slice(0, 5);
  const maximum = Math.max(...visibleItems.map((item) => item.value), 1);
  return <section className="shared-metric-panel"><div><h3>{title}</h3><span>{summary ?? (visibleItems.length ? `${visibleItems.length} categories` : "No data yet")}</span></div>{visibleItems.length ? <div className="shared-metric-bars">{visibleItems.map((item) => <div className="shared-metric-bar" key={item.label}><span>{item.label}</span><div aria-label={`${item.label}: ${formatValue(item.value)}`} className="shared-metric-bar-track"><i style={{ width: `${(item.value / maximum) * 100}%` }} /></div><strong>{formatValue(item.value)}</strong></div>)}</div> : <p className="shared-metric-empty">{emptyLabel}</p>}</section>;
}

function MetricTimeline({ items, title = "Activity, last 14 days" }: { items: WorkspaceMetricBreakdown[]; title?: string }) {
  const maximum = Math.max(...items.map((item) => item.value), 1);
  return <section className="shared-metric-panel shared-metric-timeline"><div><h3>{title}</h3><span>{items.reduce((total, item) => total + item.value, 0)} changes</span></div><div className="shared-timeline-bars" aria-label={`${title} activity graph`}>{items.map((item) => <div key={item.label}><span title={`${formatMetricDay(item.label)}: ${item.value} changes`} style={{ height: `${Math.max((item.value / maximum) * 100, item.value ? 8 : 2)}%` }} /><small>{formatMetricDay(item.label)}</small></div>)}</div></section>;
}

function ContributionCalendar({ items, title, total }: { items: WorkspaceMetricBreakdown[]; title: string; total?: number }) {
  const calendarDays = items.map((item) => ({ ...item, date: new Date(`${item.label}T12:00:00`) })).filter((item) => !Number.isNaN(item.date.getTime()));
  const maximum = Math.max(...calendarDays.map((item) => item.value), 1);
  const startOffset = calendarDays[0]?.date.getDay() ?? 0;
  const contributionTotal = total ?? calendarDays.reduce((sum, item) => sum + item.value, 0);
  const monthMarkers = calendarDays.filter((item, index) => index === 0 || item.date.getDate() === 1);
  const gridPosition = (index: number) => ({ gridColumnStart: Math.floor((startOffset + index) / 7) + 1, gridRowStart: ((startOffset + index) % 7) + 1 });
  const levelFor = (value: number) => value === 0 ? 0 : Math.max(1, Math.min(4, Math.ceil((value / maximum) * 4)));
  return <section className="shared-contribution-calendar">
    <div className="shared-contribution-heading"><div><h3>{contributionTotal.toLocaleString()} contributions in the last year</h3><span>{title}</span></div></div>
    <div className="shared-contribution-scroll">
      <div className="shared-contribution-months" aria-hidden="true">{monthMarkers.map((item) => { const index = calendarDays.indexOf(item); return <span key={item.label} style={{ gridColumnStart: gridPosition(index).gridColumnStart }}>{item.date.toLocaleDateString(undefined, { month: "short" })}</span>; })}</div>
      <div className="shared-contribution-body"><div className="shared-contribution-weekdays" aria-hidden="true"><span>Sun</span><span></span><span>Tue</span><span></span><span>Thu</span><span></span><span>Sat</span></div><div aria-label={`${title}: ${contributionTotal} contributions in the last year`} className="shared-contribution-grid" role="img">{calendarDays.map((item, index) => <span aria-label={`${formatMetricDay(item.label)}: ${item.value} contributions`} data-level={levelFor(item.value)} key={item.label} style={gridPosition(index)} title={`${formatMetricDay(item.label)}: ${item.value} contributions`} />)}</div></div>
      <div className="shared-contribution-footer"><span>Contribution activity is based on saved evidence, memory, indexing, and workspace changes.</span><div aria-hidden="true" className="shared-contribution-legend"><span>Less</span>{[0, 1, 2, 3, 4].map((level) => <i data-level={level} key={level} />)}<span>More</span></div></div>
    </div>
  </section>;
}

function formatMetricDay(value: string) {
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function artifactTypeLabel(type: ArtifactType) {
  return type.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function formatActivityTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function isTaskOverdue(task: CollaborationTask) {
  if (!task.due_at || task.status === "done") return false;
  const dueAt = new Date(task.due_at);
  return !Number.isNaN(dueAt.getTime()) && dueAt.getTime() < Date.now();
}

function formatTaskDueDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function formatProfileTime(value: string | null | undefined) {
  if (!value) return "Not recorded yet";
  return formatActivityTime(value);
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
