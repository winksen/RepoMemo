import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import {
  IconBell as Bell,
  IconLogout as Logout,
  IconMoon as Moon,
  IconServer as Server,
  IconShieldLock as Shield,
  IconSun as Sun,
  IconUserCircle as UserCircle,
} from "@tabler/icons-react";
import type { SharedSession } from "../types";
import { Button } from "./ui/button";

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
  children,
  session,
  sidebar,
  signOut,
  onNavigate,
  workspaceNavigation,
}: {
  apiAvailable: boolean | null;
  children: ReactNode;
  session: SharedSession;
  sidebar: ReactNode;
  signOut: () => void;
  onNavigate: (to: string) => void;
  workspaceNavigation?: ReactNode;
}) {
  return <main className="shared-home">
    <header className="shared-home-header">
      <Button aria-label="Go to dashboard" className="shared-brand" onClick={() => onNavigate("/dashboard")} type="button" variant="secondary"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></Button>
      <div className="shared-header-actions">
        <div className="shared-user-menu" aria-label="Account controls">
          <div className="shared-account-controls">
            <ThemeToggle />
            <Button aria-label="Open notifications" className="shared-notifications-link" onClick={() => onNavigate("/notifications")} title="Notifications" type="button" variant="secondary"><Bell size={16} /></Button>
            <Button className="shared-profile-link" onClick={() => onNavigate("/profile")} type="button" variant="secondary"><UserCircle size={16} /> Profile</Button>
          </div>
          <Button className="shared-user-signout" onClick={signOut} type="button" variant="secondary"><Logout size={16} /> Sign out</Button>
        </div>
      </div>
    </header>
    <div className="shared-home-frame">
      <aside className="shared-home-rail">{sidebar}{session.is_system_admin || session.is_app_admin ? <SystemRailEntry onNavigate={onNavigate} /> : null}<div className="shared-rail-footer"><Shield size={15} /><span>JWT active · API {apiAvailable === true ? "healthy" : apiAvailable === false ? "offline" : "checking"}</span></div></aside>
      <section className="shared-home-content">
        {workspaceNavigation}
        <div className="shared-layout-content">{children}</div>
      </section>
    </div>
  </main>;
}

/** The System entry at the bottom of the rail, for system administrators. Highlighted on every System page. */
function SystemRailEntry({ onNavigate }: { onNavigate: (to: string) => void }) {
  const isActive = window.location.pathname === "/system" || window.location.pathname.startsWith("/system/");
  return <section aria-label="System administration" className="shared-rail-system">
    <p className="shared-eyebrow">Administration</p>
    <Button aria-current={isActive ? "page" : undefined} className={`shared-rail-organization${isActive ? " active" : ""}`} onClick={() => onNavigate("/system/overview")} title="Administer this server" type="button" variant="secondary"><Server size={16} /><span>System</span></Button>
  </section>;
}
