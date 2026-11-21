import { ORGANIZATION_SECTIONS, SYSTEM_SECTIONS, WORKSPACE_SECTIONS, type OrganizationSection, type SystemSection, type WorkspaceSection } from "./constants";



export function currentPathname() {
  return window.location.pathname.replace(/\/+$/, "") || "/";
}

/** The secret of an invitation or reset link, from the URL fragment. */
export function currentLinkSecret() {
  try { return decodeURIComponent(window.location.hash.replace(/^#/, "")).trim(); } catch { return ""; }
}

export function currentLocation() {
  return `${currentPathname()}${window.location.search}`;
}

/** Asked before the address changes to `to`; resolves `false` to stay (a form with unsaved changes). */
export type NavigationGuard = (to: string) => Promise<boolean>;
const navigationGuards = new Set<NavigationGuard>();

/** Registers a guard and returns the function that removes it. */
export function addNavigationGuard(guard: NavigationGuard): () => void {
  navigationGuards.add(guard);
  return () => { navigationGuards.delete(guard); };
}

function go(to: string, replace: boolean) {
  window.history[replace ? "replaceState" : "pushState"]({}, "", to);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

/**
 * Moves to `to`. A move the person made (not a `replace`, which is the app redirecting) first
 * asks every registered guard, so a form with unsaved changes can stop it.
 * The browser's back button is not guarded: the address has already changed when the app hears of it.
 */
export function navigate(to: string, replace = false) {
  if (currentLocation() === to) return;
  if (replace || navigationGuards.size === 0) { go(to, replace); return; }
  void (async () => {
    for (const guard of [...navigationGuards]) if (!(await guard(to))) return;
    go(to, replace);
  })();
}

/** Everything the address bar can name once someone is signed in. */
export type Route =
  | { name: "dashboard" }
  | { name: "orgs" }
  /** The assistant on a page of its own; `workspaceId` is the workspace it is talking about, when the address names one. */
  | { name: "assistant"; workspaceId: string | null }
  | { name: "organization"; organizationId: string; section: OrganizationSection }
  | { name: "new-organization" }
  | { name: "profile" }
  | { name: "notifications" }
  | { name: "system"; section: SystemSection }
  | { name: "workspace"; workspaceId: string; section: WorkspaceSection; landing?: true }
  | { name: "artifact"; workspaceId: string; artifactId: string }
  | { name: "memory-card"; workspaceId: string; cardId: string }
  /** An address that names nothing; `workspaceId` is set when it sits under a workspace. */
  | { name: "not-found"; workspaceId: string | null };

function decodePart(part: string): string {
  try { return decodeURIComponent(part); } catch { return part; }
}

/** Reads a pathname (`/spaces/<id>/memory`, `/system/logs`, …) into a route. */
export function parseRoute(pathname: string): Route {
  const parts = pathname.split("/").filter(Boolean).map(decodePart);
  const [first, second, third, fourth] = parts;
  if (parts.length === 0 || (parts.length === 1 && (first === "spaces" || first === "workspaces"))) return { name: "dashboard" };
  if (parts.length === 1) {
    if (first === "dashboard") return { name: "dashboard" };
    if (first === "orgs") return { name: "orgs" };
    if (first === "assistant") return { name: "assistant", workspaceId: null };
    if (first === "profile") return { name: "profile" };
    if (first === "notifications") return { name: "notifications" };
    if (first === "system") return { name: "system", section: "overview" };
    return { name: "not-found", workspaceId: null };
  }
  if (first === "assistant") {
    return parts.length === 2 ? { name: "assistant", workspaceId: second } : { name: "not-found", workspaceId: null };
  }
  if (first === "orgs") {
    if (parts.length === 2) return second === "new" ? { name: "new-organization" } : { name: "organization", organizationId: second, section: "overview" };
    if (parts.length === 3 && second !== "new" && (ORGANIZATION_SECTIONS as readonly string[]).includes(third)) {
      return { name: "organization", organizationId: second, section: third as OrganizationSection };
    }
    return { name: "not-found", workspaceId: null };
  }
  if (first === "system") {
    if (parts.length === 2 && (SYSTEM_SECTIONS as readonly string[]).includes(second)) {
      return { name: "system", section: second as SystemSection };
    }
    return { name: "not-found", workspaceId: null };
  }
  // `/workspaces/...` is the address these pages had before V0.8.24; it is still read, and rewritten by `legacyRedirect`.
  if (first === "spaces" || first === "workspaces") {
    // No section named: the workspace opens on its template's landing section (`landing` says to look it up).
    if (parts.length === 2) return { name: "workspace", workspaceId: second, section: "overview", landing: true };
    if (parts.length === 3 && (WORKSPACE_SECTIONS as readonly string[]).includes(third)) {
      return { name: "workspace", workspaceId: second, section: third as WorkspaceSection };
    }
    if (parts.length === 4 && third === "artifacts") return { name: "artifact", workspaceId: second, artifactId: fourth };
    if (parts.length === 4 && third === "memory-cards") return { name: "memory-card", workspaceId: second, cardId: fourth };
    return { name: "not-found", workspaceId: second };
  }
  return { name: "not-found", workspaceId: null };
}

/** The address that opens a workspace on its landing section: no section named. */
export function workspaceHomePath(workspaceId: string): string {
  return `/spaces/${encodeURIComponent(workspaceId)}`;
}

/** The pathname of a route; the inverse of [`parseRoute`]. */
export function routePath(route: Exclude<Route, { name: "not-found" }>): string {
  const encode = encodeURIComponent;
  switch (route.name) {
    case "dashboard": return "/dashboard";
    case "orgs": return "/orgs";
    case "assistant": return assistantPath(route.workspaceId);
    case "organization": return organizationPath(route.organizationId, route.section);
    case "new-organization": return "/orgs/new";
    case "profile": return "/profile";
    case "notifications": return "/notifications";
    case "system": return `/system/${route.section}`;
    case "workspace": return `/spaces/${encode(route.workspaceId)}/${route.section}`;
    case "artifact": return `/spaces/${encode(route.workspaceId)}/artifacts/${encode(route.artifactId)}`;
    case "memory-card": return `/spaces/${encode(route.workspaceId)}/memory-cards/${encode(route.cardId)}`;
  }
}

/** The address of an organization's page; its Overview is the organization's address itself. */
export function organizationPath(organizationId: string, section: OrganizationSection = "overview"): string {
  return `/orgs/${encodeURIComponent(organizationId)}${section === "overview" ? "" : `/${section}`}`;
}

/**
 * Where an address of the old shape belongs now (`/workspaces/<id>/...` is `/spaces/<id>/...`, and
 * `/workspaces?organization=<id>` is `/orgs/<id>`), or `null` for an address that is already current.
 */
export function legacyRedirect(pathname: string, search: string): string | null {
  if (pathname !== "/workspaces" && !pathname.startsWith("/workspaces/")) return null;
  if (pathname === "/workspaces") {
    const parameters = new URLSearchParams(search);
    const organization = parameters.get("organization");
    if (organization) return organizationPath(organization);
    if (parameters.get("createOrganization") === "1") return "/orgs/new";
    return "/spaces";
  }
  return `/spaces${pathname.slice("/workspaces".length)}${search}`;
}

/** The address of the assistant page, for a workspace when one is known. */
export function assistantPath(workspaceId: string | null): string {
  return workspaceId ? `/assistant/${encodeURIComponent(workspaceId)}` : "/assistant";
}

/** Where the assistant button of the top bar goes from `pathname`: to the workspace being viewed, else to the page's own choice. */
export function assistantEntryPath(pathname: string): string {
  const route = parseRoute(pathname);
  return assistantPath(route.name === "workspace" || route.name === "artifact" || route.name === "memory-card" ? route.workspaceId : route.name === "assistant" ? route.workspaceId : null);
}
