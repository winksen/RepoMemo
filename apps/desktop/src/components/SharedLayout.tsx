import { createContext, type ReactNode, useContext, useEffect, useLayoutEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { applyWorkspaceColor } from "../web/workspaceColors";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { parseRoute } from "../web/router";
import { rememberCairnWorkspace } from "../web/cairn";
import { CairnIcon } from "./CairnIcon";
import { Bell, Buildings as Building, CaretDown as ChevronDown, MagnifyingGlass as Search, SignOut as Logout, Moon, HardDrives as Server, LockKey as Shield, Sun, UserCircle } from "@phosphor-icons/react";
import type { SharedSession } from "../types";
import { openCommandPalette } from "../lib/assistantBridge";
import { MAIN_CONTENT_ID } from "../web/RouteAnnouncer";
import { Button } from "./ui/button";
import { IconButton } from "./ui/icon-button";
import { Tooltip } from "./ui/tooltip";
import { CreateMenu } from "./CreateMenu";
import { DesktopServerNote } from "./DesktopServerNote";
import { lastSignIn } from "../lib/signedIn";
import { ICON_SIZE } from "./ui/icon";

const THEME_STORAGE_KEY = "repomemo.theme";
const THEME_COOKIE_MAX_AGE = 60 * 60 * 24 * 365;
export type Theme = "light" | "dark";

function readThemeCookie(): Theme | null {
  const match = document.cookie.split("; ").find((entry) => entry.startsWith(`${THEME_STORAGE_KEY}=`));
  const value = match?.slice(THEME_STORAGE_KEY.length + 1);
  return value === "light" || value === "dark" ? value : null;
}

export function initialSharedTheme(): Theme {
  const cookieTheme = readThemeCookie();
  if (cookieTheme) return cookieTheme;
  const storedTheme = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (storedTheme === "light" || storedTheme === "dark") return storedTheme;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** Theme state persisted in a cookie and applied to the document root. */
export function useSharedTheme(): [Theme, () => void] {
  const [theme, setTheme] = useState<Theme>(initialSharedTheme);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.cookie = `${THEME_STORAGE_KEY}=${theme}; path=/; max-age=${THEME_COOKIE_MAX_AGE}; SameSite=Lax`;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  return [theme, () => setTheme((current) => current === "dark" ? "light" : "dark")];
}

export function ThemeToggle({ className = "" }: { className?: string }) {
  const [theme, toggleTheme] = useSharedTheme();
  const next = theme === "dark" ? "light" : "dark";
  return <IconButton className={`shared-theme-toggle ${className}`.trim()} label={`Switch to ${next} mode`} onClick={toggleTheme} size="sm">
    {theme === "dark" ? <Sun size={ICON_SIZE.md} /> : <Moon size={ICON_SIZE.md} />}
  </IconButton>;
}

/** The elements of the persistent shell that a page's parts are drawn into (see `SharedShell`). */
interface ShellSlots { navigation: HTMLElement | null; content: HTMLElement | null; aside: HTMLElement | null }
const ShellContext = createContext<ShellSlots | null>(null);

/**
 * The frame of every signed-in page: the top bar, the rail and the empty slots of the main area. It is the same
 * for all of them, so `SharedWebApp` keeps ONE copy mounted while the page inside it changes, and moving from page
 * to page redraws only the page, never the bar or the rail (V0.8.25).
 */
function LayoutFrame({ chatsOnly = false, session, sidebar, signOut, onNavigate, defaultOrganizationId, navigation, aside, children, slots }: {
  /** The rail holds only what `sidebar` gives it: no Organizations, System, session or server notes (Cairn's page). */
  chatsOnly?: boolean;
  session: SharedSession;
  sidebar: ReactNode;
  signOut: () => void;
  onNavigate: (to: string) => void;
  defaultOrganizationId?: string;
  navigation?: ReactNode;
  aside?: ReactNode;
  children?: ReactNode;
  /** Set in the persistent shell: the page's parts are drawn into these slots instead of being given here. */
  slots?: { setNavigation: (element: HTMLElement | null) => void; setContent: (element: HTMLElement | null) => void; setAside: (element: HTMLElement | null) => void };
}) {
  // On a narrow screen the rail folds away behind a button so the page starts at the top; wide screens always show it.
  const [railOpen, setRailOpen] = useState(false);
  const onCairn = window.location.pathname === "/cairn" || window.location.pathname.startsWith("/cairn/");
  // From a workspace page Cairn starts its next chat in that workspace.
  function openCairn() {
    const route = parseRoute(window.location.pathname);
    if (route.name === "workspace" || route.name === "artifact" || route.name === "memory-card") rememberCairnWorkspace(route.workspaceId);
    onNavigate("/cairn");
  }
  return <div className="shared-home">
    <a className="rm-skip-link" href={`#${MAIN_CONTENT_ID}`} onClick={(event) => { event.preventDefault(); document.getElementById(MAIN_CONTENT_ID)?.focus(); }}>Skip to the page</a>
    <header className="shared-home-header">
      <Button aria-label="Go to dashboard" className="shared-brand" onClick={() => onNavigate("/dashboard")} type="button" variant="secondary"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></Button>
      <div className="shared-header-actions">
        <div className="shared-user-menu" aria-label="Account controls" role="group">
          <div className="shared-account-controls">
            <CreateMenu canCreateOrganization={session.can_create_organizations !== false} defaultOrganizationId={defaultOrganizationId} />
            <ThemeToggle />
            <IconButton aria-current={onCairn ? "page" : undefined} className="shared-assistant-link" label={ASSISTANT_NAME} onClick={openCairn} size="sm"><CairnIcon size={ICON_SIZE.md} weight={onCairn ? "fill" : "regular"} /></IconButton>
            <IconButton className="shared-palette-link" label="Open command palette" onClick={openCommandPalette} size="sm"><Search size={ICON_SIZE.md} /></IconButton>
            <IconButton className="shared-notifications-link" label="Notifications" onClick={() => onNavigate("/notifications")} size="sm"><Bell size={ICON_SIZE.md} /></IconButton>
            <Button className="shared-profile-link" onClick={() => onNavigate("/profile")} type="button" variant="secondary"><UserCircle size={ICON_SIZE.md} /> <span className="shared-button-label">Profile</span></Button>
          </div>
          <Button className="shared-user-signout" onClick={signOut} type="button" variant="secondary"><Logout size={ICON_SIZE.md} /> <span className="shared-button-label">Sign out</span></Button>
        </div>
      </div>
    </header>
    <div className="shared-home-frame" data-wide={chatsOnly ? "true" : undefined}>
      <aside aria-label="Workspaces and organizations" className="shared-home-rail" data-open={railOpen}>
        <Button aria-controls="shared-rail-body" aria-expanded={railOpen} className="shared-rail-toggle" onClick={() => setRailOpen((open) => !open)} type="button" variant="secondary"><span>Workspaces and organizations</span><ChevronDown size={ICON_SIZE.md} /></Button>
        <div className="shared-rail-body" id="shared-rail-body">{sidebar}{chatsOnly ? null : <><AdministrationRail onNavigate={onNavigate} showSystem={Boolean(session.is_system_admin || session.is_app_admin)} /><SessionNote /><DesktopServerNote signedIn /></>}</div>
      </aside>
      <main className="shared-home-content" id={MAIN_CONTENT_ID} tabIndex={-1}>
        {slots ? <div className="shared-shell-slot" ref={slots.setNavigation} /> : navigation}
        <div className="shared-layout-body">
          {slots ? <div className="shared-layout-content" ref={slots.setContent} /> : <div className="shared-layout-content">{children}</div>}
          {slots ? <div className="shared-shell-slot" ref={slots.setAside} /> : aside}
        </div>
      </main>
    </div>
  </div>;
}

/**
 * Keeps the frame mounted across pages. The pages below it still render `SharedLayout`, which draws their parts
 * into the frame's slots; the rail comes from here, so it does not blink while a page loads.
 * `scrollKey` changes with the page: the content starts at the top again.
 */
export function SharedShell({ chatsOnly = false, children, defaultOrganizationId, scrollKey, session, sidebar, signOut, onNavigate }: {
  chatsOnly?: boolean;
  children: ReactNode;
  defaultOrganizationId?: string;
  scrollKey: string;
  session: SharedSession;
  sidebar: ReactNode;
  signOut: () => void;
  onNavigate: (to: string) => void;
}) {
  const [navigation, setNavigation] = useState<HTMLElement | null>(null);
  const [content, setContent] = useState<HTMLElement | null>(null);
  const [aside, setAside] = useState<HTMLElement | null>(null);
  const slots = useMemo(() => ({ navigation, content, aside }), [navigation, content, aside]);
  useEffect(() => { content?.scrollTo?.({ top: 0 }); }, [content, scrollKey]);
  return <ShellContext.Provider value={slots}>
    <LayoutFrame chatsOnly={chatsOnly} defaultOrganizationId={defaultOrganizationId} onNavigate={onNavigate} session={session} sidebar={sidebar} signOut={signOut} slots={{ setNavigation, setContent, setAside }} />
    {children}
  </ShellContext.Provider>;
}

/** A page's frame. Inside the shell it draws the page's parts into the shell's slots; on its own (tests, pages outside the shell) it is the whole frame. */
export function SharedLayout({
  accent = null,
  aside,
  children,
  session,
  sidebar,
  signOut,
  onNavigate,
  workspaceNavigation,
}: {
  /** The workspace colour (`workspaceColorKey`): the tabs and the page take it as their highlight. The rail and the bar keep the app's own. */
  accent?: string | null;
  /** Not used any more (the rail no longer reports the API); kept so existing callers need no change. */
  apiAvailable?: boolean | null;
  /** A section docked on the right of the page content, below the workspace navigation. */
  aside?: ReactNode;
  children: ReactNode;
  session: SharedSession;
  sidebar: ReactNode;
  signOut: () => void;
  onNavigate: (to: string) => void;
  workspaceNavigation?: ReactNode;
}) {
  const slots = useContext(ShellContext);
  useLayoutEffect(() => {
    if (!slots) return;
    const parts = [slots.navigation, slots.content, slots.aside];
    for (const part of parts) applyWorkspaceColor(part, accent);
    return () => { for (const part of parts) applyWorkspaceColor(part, null); };
  }, [slots, accent]);
  if (!slots) return <LayoutFrame aside={aside} navigation={workspaceNavigation} onNavigate={onNavigate} session={session} sidebar={sidebar} signOut={signOut}>{children}</LayoutFrame>;
  return <>
    {slots.navigation && workspaceNavigation ? createPortal(workspaceNavigation, slots.navigation) : null}
    {slots.content ? createPortal(children, slots.content) : null}
    {slots.aside && aside ? createPortal(aside, slots.aside) : null}
  </>;
}

/** Draws `children` into the page area of the shell, for a page that has no frame props of its own (the workspace loader). */
export function ShellContent({ children }: { children: ReactNode }) {
  const slots = useContext(ShellContext);
  if (!slots) return <>{children}</>;
  return slots.content ? createPortal(children, slots.content) : null;
}

/** The bottom of the rail: every organization (everyone) and the System pages (administrators). Each is highlighted on its own pages. */
function AdministrationRail({ onNavigate, showSystem }: { onNavigate: (to: string) => void; showSystem: boolean }) {
  const path = window.location.pathname;
  const inOrganizations = path === "/orgs" || path.startsWith("/orgs/");
  const inSystem = path === "/system" || path.startsWith("/system/");
  return <section aria-label="Administration" className="shared-rail-system">
    <Tooltip content="Manage your organizations" side="right"><Button aria-current={inOrganizations ? "page" : undefined} className={`shared-rail-organization${inOrganizations ? " active" : ""}`} onClick={() => onNavigate("/orgs")} type="button" variant="secondary"><Building size={ICON_SIZE.lg} weight={inOrganizations ? "fill" : "regular"} /><span>Organizations</span></Button></Tooltip>
    {showSystem ? <Tooltip content="Administer this server" side="right"><Button aria-current={inSystem ? "page" : undefined} className={`shared-rail-organization${inSystem ? " active" : ""}`} onClick={() => onNavigate("/system/overview")} type="button" variant="secondary"><Server size={ICON_SIZE.lg} weight={inSystem ? "fill" : "regular"} /><span>System</span></Button></Tooltip> : null}
  </section>;
}

/** The foot of the rail: the session is connected, and when this browser signed in, if it knows. */
function SessionNote() {
  const since = lastSignIn();
  return <div className="shared-rail-footer"><Shield size={ICON_SIZE.md} /><span>Session connected{since ? <small>Signed in {since.toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}</small> : null}</span></div>;
}
