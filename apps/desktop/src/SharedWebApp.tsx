/*
THESIS: Shared mode is a clear trust boundary, not a local-preview replica; identity precedes workspace access.
OWN-WORLD: The existing evidence instrument stays light and precise, with white surfaces, quiet seams, and reserved green-blue action.
STORY: Sign in, establish an organization, and create a server-authoritative workspace without mistaking local data for shared data.
FIRST VIEWPORT: A calm explanatory field balances a focused access form; after authentication, an organization rail frames the workspace ledger.
FORM: The established RepoMemo operate composition, adapted into a protected browser session with direct API-state feedback.
*/

import { useEffect, useState } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "./lib/queryClient";
import { SHARED_SESSION_STORAGE_KEY as SESSION_STORAGE_KEY, clearSharedSession, getPlatformStatus, getServerSetup, getSharedHealth, getSharedSession, listSharedOrganizations, listSharedWorkspaces, refreshSharedSession } from "./lib/sharedApi";
import { type Organization, type ServerSetupStatus, type SharedSession, type SharedWorkspace } from "./types";
import { PlatformSetup } from "./components/PlatformSetup";
import { ServerSetup, ServerSetupWaiting } from "./components/ServerSetup";
import { InvitePage } from "./components/InvitePage";
import { PasswordResetPage } from "./components/PasswordResetPage";
import { VerifyEmailPage } from "./components/VerifyEmailPage";
import { SharedLayout, initialSharedTheme } from "./components/SharedLayout";
import { type PageState } from "./web/constants";
import { apiMessage } from "./web/format";
import { currentLinkSecret, currentLocation, currentPathname, navigate, parseRoute, routePath } from "./web/router";
import { SystemArea } from "./web/lazy";
import { OrganizationNavigationProvider } from "./web/navigation";
import { LoadingCanvas } from "./web/common";
import { AuthCanvas } from "./web/AuthCanvas";
import { SharedProfile } from "./web/Profile";
import { SharedNotifications } from "./web/Notifications";
import { SharedDashboard, SharedWorkspaceHome } from "./web/Dashboard";
import { SharedWorkspaceDetail } from "./web/WorkspaceDetail";
import { SharedArtifactDetail } from "./web/ArtifactDetail";
import { SharedMemoryCardDetail, SharedRouteNotFound } from "./web/MemoryCardDetail";
import { CommandPalette } from "./web/CommandPalette";
import { RouteAnnouncer } from "./web/RouteAnnouncer";
import { pageKey, pageTitle } from "./web/pageTitle";

export function SharedWebApp() {
  return (
    <QueryClientProvider client={queryClient}>
      <OrganizationNavigationProvider><SharedWebAppContent /></OrganizationNavigationProvider>
    </QueryClientProvider>
  );
}

function SharedWebAppContent() {
  const [pageState, setPageState] = useState<PageState>("restoring");
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [session, setSession] = useState<SharedSession | null>(null);
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [workspaces, setWorkspaces] = useState<SharedWorkspace[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pathname, setPathname] = useState(currentPathname);
  // Invitation and reset links carry their secret in the fragment (`/invite#<secret>`), never sent to a server.
  const [linkSecret, setLinkSecret] = useState(currentLinkSecret);
  const [apiAvailable, setApiAvailable] = useState<boolean | null>(null);
  // Whether the server still has to be set up; the server is the authority.
  const [setup, setSetup] = useState<ServerSetupStatus | null>(null);
  // Whether the server has an environment folder attached; without one it can only show the environment menu.
  const [platform, setPlatform] = useState<{ attached: boolean; folder: string; pinned_environment?: string | null } | null>(null);

  useEffect(() => {
    // An older server has no platform routes and is always attached.
    getPlatformStatus().then(setPlatform).catch(() => setPlatform({ attached: true, folder: "workspace-data" }));
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = initialSharedTheme();
  }, []);

  useEffect(() => {
    // If the status cannot be read (an older server, or the API is down), the normal sign-in shows.
    getServerSetup().then(setSetup).catch(() => setSetup({ state: "complete", onboarding: false }));
  }, []);

  useEffect(() => {
    // The secret was taken out of the address (below), so an empty fragment keeps the one already read.
    const syncPathname = () => { setPathname(currentPathname()); setLinkSecret((read) => currentLinkSecret() || read); };
    window.addEventListener("popstate", syncPathname);
    window.addEventListener("hashchange", syncPathname);
    return () => { window.removeEventListener("popstate", syncPathname); window.removeEventListener("hashchange", syncPathname); };
  }, []);

  useEffect(() => {
    // The secret of a link page is held in memory only: take it out of the address bar and the history entry (review L3), and drop it once the person leaves the page.
    if (pathname === "/invite" || pathname === "/reset-password") {
      if (window.location.hash) window.history.replaceState(window.history.state, "", currentLocation());
    } else if (linkSecret) {
      setLinkSecret("");
    }
  }, [pathname, linkSecret]);

  useEffect(() => {
    // Links made by V0.1.63 carried the secret in the path (`/invite/<secret>`); keep them working, with the secret moved to the fragment.
    const legacyLink = /^\/(invite|reset-password)\/([^/]+)$/.exec(pathname);
    if (legacyLink) navigate(`/${legacyLink[1]}#${legacyLink[2]}`, true);
  }, [pathname]);

  useEffect(() => {
    getSharedHealth().then(() => setApiAvailable(true)).catch(() => setApiAvailable(false));
  }, []);

  useEffect(() => {
    function showSignIn() {
      if (currentPathname() === "/" || currentPathname() === "/dashboard" || currentPathname().startsWith("/workspaces") || currentPathname().startsWith("/system")) {
        navigate("/login", true);
      }
      setPageState("unauthenticated");
    }
    function restore(token: string) {
      hydrate(token).catch(() => {
        clearSharedSession();
        navigate("/login", true);
        setPageState("unauthenticated");
      });
    }
    const storedToken = window.sessionStorage.getItem(SESSION_STORAGE_KEY);
    if (storedToken) {
      restore(storedToken);
      return;
    }
    // New tab or reload: a surviving refresh token can still mint an access token.
    refreshSharedSession().then((token) => (token ? restore(token) : showSignIn()));
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

  function handleAuthenticated(token: string, destination = "/dashboard") {
    window.sessionStorage.setItem(SESSION_STORAGE_KEY, token);
    navigate(destination, true);
    hydrate(token).catch(() => undefined);
  }

  function signOut() {
    clearSharedSession();
    setAccessToken(null);
    setSession(null);
    setOrganizations([]);
    setWorkspaces([]);
    setError(null);
    setPageState("unauthenticated");
    navigate("/login", true);
  }

  useEffect(() => {
    if (setup?.onboarding) {
      if (pathname !== "/setup") navigate("/setup", true);
      return;
    }
    // The setup page exists only while the server is being set up.
    if (setup && pathname === "/setup") navigate(pageState === "ready" ? "/dashboard" : "/login", true);
    if (pageState === "ready" && (pathname === "/" || pathname === "/login" || pathname === "/register")) {
      navigate("/dashboard", true);
    }
  }, [pageState, pathname, setup]);

  if (!platform) {
    return <LoadingCanvas label="Checking the server" />;
  }

  if (!platform.attached) {
    // Everything else on the page was loaded against a server with no data; start over once one is attached.
    return <PlatformSetup folder={platform.folder} onAttached={() => window.location.assign("/")} pinned={platform.pinned_environment ?? null} />;
  }

  if (!setup) {
    return <LoadingCanvas label="Checking the server" />;
  }

  if (setup.onboarding && setup.state === "new") {
    return <ServerSetup
      onAdminCreated={(token) => {
        window.sessionStorage.setItem(SESSION_STORAGE_KEY, token);
        setSetup({ state: "finishing", onboarding: true });
        hydrate(token).catch(() => undefined);
      }}
      phase="new"
    />;
  }

  if (setup.onboarding && pageState === "ready" && accessToken && session) {
    if (!session.is_system_admin) return <ServerSetupWaiting onSignOut={signOut} />;
    return <ServerSetup
      accessToken={accessToken}
      onFinished={() => {
        setSetup({ state: "complete", onboarding: false });
        navigate("/dashboard", true);
        // Reload organizations and workspaces created during setup.
        hydrate(accessToken).catch(() => undefined);
      }}
      onSignOut={signOut}
      phase="finishing"
      session={session}
    />;
  }

  if (pageState === "restoring") {
    return <LoadingCanvas label="Restoring your shared session" />;
  }

  // Links people receive: they work signed in or not.
  if (pathname === "/invite" && linkSecret) {
    const signedIn = pageState === "ready" && accessToken && session;
    return <InvitePage
      accessToken={signedIn ? accessToken : null}
      onAuthenticated={(token, destination) => handleAuthenticated(token, destination)}
      onJoined={(destination) => { navigate(destination, true); if (accessToken) hydrate(accessToken).catch(() => undefined); }}
      onSignOut={() => { clearSharedSession(); window.location.reload(); }}
      session={signedIn ? session : null}
      token={linkSecret}
    />;
  }
  if (pathname === "/verify-email" && linkSecret) {
    return <VerifyEmailPage onContinue={() => { window.location.assign(accessToken ? "/profile" : "/login"); }} token={linkSecret} />;
  }
  if (pathname === "/reset-password" && linkSecret) {
    return <PasswordResetPage onAuthenticated={(token) => handleAuthenticated(token)} token={linkSecret} />;
  }

  if (pageState === "unauthenticated" || pageState === "error") {
    return (
      <AuthCanvas
        setupPending={setup.onboarding}
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

  const route = parseRoute(pathname);
  const routedWorkspaceId = route.name === "workspace" || route.name === "artifact" || route.name === "memory-card"
    ? route.workspaceId
    : route.name === "not-found" ? route.workspaceId : null;
  const workspace = routedWorkspaceId
    ? workspaces.find((entry) => entry.workspace.id === routedWorkspaceId)
    : undefined;
  const workspaceOrganization = workspace
    ? organizations.find((organization) => organization.id === workspace.organization_id)
    : undefined;
  const openWorkspace = (id: string) => navigate(routePath({ name: "workspace", workspaceId: id, section: "overview" }));
  const openArtifact = (artifactId: string) => navigate(routePath({ name: "artifact", workspaceId: routedWorkspaceId!, artifactId }));
  const notFound = (backTo: string, scope: { workspace?: SharedWorkspace; organization?: Organization } = {}) => (
    <SharedRouteNotFound apiAvailable={apiAvailable} onBack={() => navigate(backTo)} organization={scope.organization} organizations={organizations} session={session} signOut={signOut} workspace={scope.workspace} workspaces={workspaces} />
  );

  const renderPage = () => {
  switch (route.name) {
    case "profile":
      return <SharedProfile accessToken={accessToken} apiAvailable={apiAvailable} organizations={organizations} onSessionUserUpdated={(user) => setSession((current) => current ? { ...current, user } : current)} session={session} signOut={signOut} workspaces={workspaces} />;

    case "system":
      if (!(session.is_system_admin || session.is_app_admin)) return notFound("/dashboard");
      return <SystemArea accessToken={accessToken} apiAvailable={apiAvailable} organizations={organizations} section={route.section} session={session} signOut={signOut} workspaces={workspaces} />;

    case "notifications":
      return <SharedNotifications accessToken={accessToken} apiAvailable={apiAvailable} organizations={organizations} session={session} signOut={signOut} workspaces={workspaces} />;

    case "dashboard":
      return <SharedDashboard accessToken={accessToken} apiAvailable={apiAvailable} organizations={organizations} onOpenWorkspace={(entry) => openWorkspace(entry.workspace.id)} session={session} signOut={signOut} workspaces={workspaces} />;

    case "artifact":
      if (!workspace) return notFound("/workspaces");
      return <SharedArtifactDetail accessToken={accessToken} apiAvailable={apiAvailable} artifactId={route.artifactId} onBack={() => openWorkspace(workspace.workspace.id)} onOpenArtifact={openArtifact} organization={workspaceOrganization} organizations={organizations} session={session} signOut={signOut} workspace={workspace} workspaces={workspaces} />;

    case "memory-card":
      if (!workspace) return notFound("/workspaces");
      return <SharedMemoryCardDetail accessToken={accessToken} apiAvailable={apiAvailable} cardId={route.cardId} onBack={() => openWorkspace(workspace.workspace.id)} onOpenArtifact={openArtifact} organization={workspaceOrganization} organizations={organizations} session={session} signOut={signOut} workspace={workspace} workspaces={workspaces} />;

    case "workspace":
      if (!workspace) return notFound("/workspaces");
      return (
        <SharedWorkspaceDetail
          accessToken={accessToken}
          apiAvailable={apiAvailable}
          onWorkspaceDeleted={(deletedWorkspaceId) => {
            setWorkspaces((current) => current.filter((entry) => entry.workspace.id !== deletedWorkspaceId));
            navigate(`/workspaces?organization=${encodeURIComponent(workspace.organization_id)}`);
          }}
          onWorkspaceUpdated={(updatedWorkspace) => {
            setWorkspaces((current) => current.map((entry) => (
              entry.workspace.id === updatedWorkspace.id ? { ...entry, workspace: updatedWorkspace } : entry
            )));
          }}
          onNavigate={(section) => navigate(routePath({ name: "workspace", workspaceId: workspace.workspace.id, section }))}
          section={route.section}
          onOpenArtifact={openArtifact}
          onOpenMemoryCard={(cardId) => navigate(routePath({ name: "memory-card", workspaceId: workspace.workspace.id, cardId }))}
          organization={workspaceOrganization}
          organizations={organizations}
          session={session}
          signOut={signOut}
          workspace={workspace}
          workspaces={workspaces}
        />
      );

    case "not-found":
      if (route.workspaceId) return notFound(routePath({ name: "workspace", workspaceId: route.workspaceId, section: "overview" }), { workspace, organization: workspaceOrganization });
      return notFound("/workspaces");

    case "workspaces":
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
          onOpenWorkspace={(entry) => openWorkspace(entry.workspace.id)}
          workspaces={workspaces}
        />
      );
  }
  };

  // The palette is on every signed-in page; the assistant can be started from it where one is docked.
  return <>
    <RouteAnnouncer pageKey={pageKey(route)} title={pageTitle(route, { workspace: workspace?.workspace.name })} />
    {renderPage()}
    <CommandPalette accessToken={accessToken} assistantWorkspaceId={route.name === "workspace" ? route.workspaceId : null} canAdministerSystem={Boolean(session.is_system_admin || session.is_app_admin)} currentWorkspaceId={workspace ? workspace.workspace.id : null} workspaces={workspaces} />
  </>;
}
