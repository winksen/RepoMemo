import { SYSTEM_SECTIONS, WORKSPACE_SECTIONS, type SystemSection, type WorkspaceSection } from "./constants";



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

export function navigate(to: string, replace = false) {
  if (currentLocation() === to) return;
  window.history[replace ? "replaceState" : "pushState"]({}, "", to);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

/** Everything the address bar can name once someone is signed in. */
export type Route =
  | { name: "workspaces" }
  | { name: "dashboard" }
  | { name: "profile" }
  | { name: "notifications" }
  | { name: "system"; section: SystemSection }
  | { name: "workspace"; workspaceId: string; section: WorkspaceSection }
  | { name: "artifact"; workspaceId: string; artifactId: string }
  | { name: "memory-card"; workspaceId: string; cardId: string }
  /** An address that names nothing; `workspaceId` is set when it sits under a workspace. */
  | { name: "not-found"; workspaceId: string | null };

function decodePart(part: string): string {
  try { return decodeURIComponent(part); } catch { return part; }
}

/** Reads a pathname (`/workspaces/<id>/memory`, `/system/logs`, …) into a route. */
export function parseRoute(pathname: string): Route {
  const parts = pathname.split("/").filter(Boolean).map(decodePart);
  const [first, second, third, fourth] = parts;
  if (parts.length === 0 || (parts.length === 1 && first === "workspaces")) return { name: "workspaces" };
  if (parts.length === 1) {
    if (first === "dashboard") return { name: "dashboard" };
    if (first === "profile") return { name: "profile" };
    if (first === "notifications") return { name: "notifications" };
    if (first === "system") return { name: "system", section: "overview" };
    return { name: "not-found", workspaceId: null };
  }
  if (first === "system") {
    if (parts.length === 2 && (SYSTEM_SECTIONS as readonly string[]).includes(second)) {
      return { name: "system", section: second as SystemSection };
    }
    return { name: "not-found", workspaceId: null };
  }
  if (first === "workspaces") {
    if (parts.length === 2) return { name: "workspace", workspaceId: second, section: "overview" };
    if (parts.length === 3 && (WORKSPACE_SECTIONS as readonly string[]).includes(third)) {
      return { name: "workspace", workspaceId: second, section: third as WorkspaceSection };
    }
    if (parts.length === 4 && third === "artifacts") return { name: "artifact", workspaceId: second, artifactId: fourth };
    if (parts.length === 4 && third === "memory-cards") return { name: "memory-card", workspaceId: second, cardId: fourth };
    return { name: "not-found", workspaceId: second };
  }
  return { name: "not-found", workspaceId: null };
}

/** The pathname of a route; the inverse of [`parseRoute`]. */
export function routePath(route: Exclude<Route, { name: "not-found" }>): string {
  const encode = encodeURIComponent;
  switch (route.name) {
    case "workspaces": return "/workspaces";
    case "dashboard": return "/dashboard";
    case "profile": return "/profile";
    case "notifications": return "/notifications";
    case "system": return `/system/${route.section}`;
    case "workspace": return `/workspaces/${encode(route.workspaceId)}/${route.section}`;
    case "artifact": return `/workspaces/${encode(route.workspaceId)}/artifacts/${encode(route.artifactId)}`;
    case "memory-card": return `/workspaces/${encode(route.workspaceId)}/memory-cards/${encode(route.cardId)}`;
  }
}
