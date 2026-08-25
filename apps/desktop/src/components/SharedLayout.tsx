import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import {
  IconBell as Bell,
  IconLayoutDashboard as Dashboard,
  IconLogout as Logout,
  IconMoon as Moon,
  IconShieldLock as Shield,
  IconStack2 as Layers,
  IconSun as Sun,
  IconUserCircle as UserCircle,
} from "@tabler/icons-react";
import type { SharedSession } from "../types";
import { Button } from "./ui/button";

const THEME_STORAGE_KEY = "repomemo.theme";
type Theme = "light" | "dark";

export function initialSharedTheme(): Theme {
  const storedTheme = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (storedTheme === "light" || storedTheme === "dark") return storedTheme;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function SharedLayout({
  apiAvailable,
  children,
  session,
  sidebar,
  signOut,
  onNavigate,
}: {
  apiAvailable: boolean | null;
  children: ReactNode;
  session: SharedSession;
  sidebar: ReactNode;
  signOut: () => void;
  onNavigate: (to: string) => void;
}) {
  const [theme, setTheme] = useState<Theme>(initialSharedTheme);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  return <main className="shared-home">
    <header className="shared-home-header">
      <div className="shared-brand"><span className="shared-brand-glyph"><Layers size={19} /></span><strong>RepoMemo</strong><span className="shared-mode-tag">Shared</span></div>
      <div className="shared-header-actions">
        <Button aria-current={window.location.pathname === "/dashboard" ? "page" : undefined} className="shared-dashboard-link" onClick={() => onNavigate("/dashboard")} type="button" variant="secondary"><Dashboard size={16} /> Dashboard</Button>
        <div className="shared-user-menu" aria-label="Account controls">
          <div className="shared-account-controls">
            <Button aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} mode`} aria-pressed={theme === "dark"} className="shared-theme-toggle" onClick={() => setTheme((current) => current === "dark" ? "light" : "dark")} title={`Switch to ${theme === "dark" ? "light" : "dark"} mode`} type="button" variant="secondary">
              {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
            </Button>
            <Button aria-label="Open notifications" className="shared-notifications-link" onClick={() => onNavigate("/notifications")} title="Notifications" type="button" variant="secondary"><Bell size={16} /></Button>
            <Button className="shared-profile-link" onClick={() => onNavigate("/profile")} type="button" variant="secondary"><UserCircle size={16} /> Profile</Button>
          </div>
          <Button className="shared-user-signout" onClick={signOut} type="button" variant="secondary"><Logout size={16} /> Sign out</Button>
        </div>
      </div>
    </header>
    <div className="shared-home-frame">
      <aside className="shared-home-rail">{sidebar}<div className="shared-rail-footer"><Shield size={15} /><span>JWT active · API {apiAvailable === true ? "healthy" : apiAvailable === false ? "offline" : "checking"}</span></div></aside>
      <section className="shared-home-content">{children}</section>
    </div>
  </main>;
}
