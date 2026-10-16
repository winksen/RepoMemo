import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import {
  IconBell as Bell,
  IconChevronDown as ChevronDown,
  IconSearch as Search,
  IconLogout as Logout,
  IconMoon as Moon,
  IconServer as Server,
  IconShieldLock as Shield,
  IconSun as Sun,
  IconUserCircle as UserCircle,
} from "@tabler/icons-react";
import type { SharedSession } from "../types";
import { openCommandPalette } from "../lib/assistantBridge";
import { MAIN_CONTENT_ID } from "../web/RouteAnnouncer";
import { Button } from "./ui/button";
import { DesktopServerNote } from "./DesktopServerNote";

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
  return <Button aria-label={`Switch to ${next} mode`} aria-pressed={theme === "dark"} className={`shared-theme-toggle ${className}`.trim()} onClick={toggleTheme} title={`Switch to ${next} mode`} type="button" variant="secondary">
    {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
  </Button>;
}

export function SharedLayout({
  apiAvailable,
  aside,
  children,
  session,
  sidebar,
  signOut,
  onNavigate,
  workspaceNavigation,
}: {
  apiAvailable: boolean | null;
  /** A section docked on the right of the page content, below the workspace navigation. */
  aside?: ReactNode;
  children: ReactNode;
  session: SharedSession;
  sidebar: ReactNode;
  signOut: () => void;
  onNavigate: (to: string) => void;
  workspaceNavigation?: ReactNode;
}) {
  // On a narrow screen the rail folds away behind a button so the page starts at the top; wide screens always show it.
  const [railOpen, setRailOpen] = useState(false);
  return <div className="shared-home">
    <a className="rm-skip-link" href={`#${MAIN_CONTENT_ID}`} onClick={(event) => { event.preventDefault(); document.getElementById(MAIN_CONTENT_ID)?.focus(); }}>Skip to the page</a>
    <header className="shared-home-header">
      <Button aria-label="Go to dashboard" className="shared-brand" onClick={() => onNavigate("/dashboard")} type="button" variant="secondary"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></Button>
      <div className="shared-header-actions">
        <div className="shared-user-menu" aria-label="Account controls" role="group">
          <div className="shared-account-controls">
            <ThemeToggle />
            <Button aria-label="Open command palette" className="shared-palette-link" onClick={openCommandPalette} title="Command palette (Ctrl K)" type="button" variant="secondary"><Search size={16} /></Button>
            <Button aria-label="Open notifications" className="shared-notifications-link" onClick={() => onNavigate("/notifications")} title="Notifications" type="button" variant="secondary"><Bell size={16} /></Button>
            <Button className="shared-profile-link" onClick={() => onNavigate("/profile")} type="button" variant="secondary"><UserCircle size={16} /> <span className="shared-button-label">Profile</span></Button>
          </div>
          <Button className="shared-user-signout" onClick={signOut} type="button" variant="secondary"><Logout size={16} /> <span className="shared-button-label">Sign out</span></Button>
        </div>
      </div>
    </header>
    <div className="shared-home-frame">
      <aside aria-label="Workspaces and organizations" className="shared-home-rail" data-open={railOpen}>
        <Button aria-controls="shared-rail-body" aria-expanded={railOpen} className="shared-rail-toggle" onClick={() => setRailOpen((open) => !open)} type="button" variant="secondary"><span>Workspaces and organizations</span><ChevronDown size={16} /></Button>
        <div className="shared-rail-body" id="shared-rail-body">{sidebar}{session.is_system_admin || session.is_app_admin ? <SystemRailEntry onNavigate={onNavigate} /> : null}<div className="shared-rail-footer"><Shield size={15} /><span>JWT active · API {apiAvailable === true ? "healthy" : apiAvailable === false ? "offline" : "checking"}</span></div><DesktopServerNote signedIn /></div>
      </aside>
      <main className="shared-home-content" id={MAIN_CONTENT_ID} tabIndex={-1}>
        {workspaceNavigation}
        <div className="shared-layout-body">
          <div className="shared-layout-content">{children}</div>
          {aside}
        </div>
      </main>
    </div>
  </div>;
}

/** The System entry at the bottom of the rail, for system administrators. Highlighted on every System page. */
function SystemRailEntry({ onNavigate }: { onNavigate: (to: string) => void }) {
  const isActive = window.location.pathname === "/system" || window.location.pathname.startsWith("/system/");
  return <section aria-label="System administration" className="shared-rail-system">
    <p className="shared-eyebrow">Administration</p>
    <Button aria-current={isActive ? "page" : undefined} className={`shared-rail-organization${isActive ? " active" : ""}`} onClick={() => onNavigate("/system/overview")} title="Administer this server" type="button" variant="secondary"><Server size={16} /><span>System</span></Button>
  </section>;
}
