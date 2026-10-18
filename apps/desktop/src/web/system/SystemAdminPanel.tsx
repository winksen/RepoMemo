import type { ReactNode } from "react";
import {
  IconAdjustments as Adjustments,
  IconChartBar as ChartBar,
  IconClipboardList as ClipboardList,
  IconFileText as FileText,
  IconServer as Server,
  IconTerminal2 as Terminal,
  IconTool as Tool,
  IconUsers as Users,
} from "@tabler/icons-react";
import { SystemConsole } from "../../components/SystemConsole";
import type { SharedSession, SharedWorkspace } from "../../types";
import { Button } from "../../components/ui/button";
import { PageHeader } from "../../components/ui/page-header";
import { SYSTEM_SECTIONS, type SystemSection } from "../constants";
import { ICON_SIZE } from "../../components/ui/icon";
import { OverviewSection } from "./Overview";
import { UsageSection } from "./Usage";
import { UsersSection } from "./Users";
import { SettingsSection } from "./Settings";
import { LogsSection } from "./Logs";
import { JobsSection } from "./Jobs";
import { AuditSection } from "./Audit";

export { SYSTEM_SECTIONS };
export type { SystemSection };

const SECTION_LABELS: Record<SystemSection, { label: string; icon: ReactNode }> = {
  overview: { label: "Overview", icon: <Server size={ICON_SIZE.md} /> },
  usage: { label: "Usage", icon: <ChartBar size={ICON_SIZE.md} /> },
  users: { label: "Users", icon: <Users size={ICON_SIZE.md} /> },
  settings: { label: "Settings", icon: <Adjustments size={ICON_SIZE.md} /> },
  logs: { label: "Logs", icon: <FileText size={ICON_SIZE.md} /> },
  jobs: { label: "Jobs & maintenance", icon: <Tool size={ICON_SIZE.md} /> },
  audit: { label: "Audit trail", icon: <ClipboardList size={ICON_SIZE.md} /> },
  console: { label: "Console", icon: <Terminal size={ICON_SIZE.md} /> },
};

/** Tabs of the System area, shown in the layout's navigation slot. */
export function SystemNavigation({ active, onNavigate }: { active: SystemSection; onNavigate: (section: SystemSection) => void }) {
  return <nav aria-label="System sections" className="shared-workspace-topbar">
    <div className="shared-workspace-tabs">
      {SYSTEM_SECTIONS.map((section) => {
        const isActive = section === active;
        return <Button aria-current={isActive ? "page" : undefined} className={isActive ? "active" : ""} key={section} onClick={() => onNavigate(section)} type="button" variant="secondary">{SECTION_LABELS[section].icon}<span className="shared-workspace-tab-label">{SECTION_LABELS[section].label}</span></Button>;
      })}
    </div>
  </nav>;
}

const SECTION_INTRO: Record<SystemSection, string> = {
  overview: "This server instance, the clients calling it, its storage and its background work.",
  usage: "How the organizations and workspaces on this server are used.",
  users: "Every account on the server. System administrators act as administrators in every organization and workspace.",
  settings: "Settings that apply to the whole server. Changes take effect at once and are recorded in the audit trail.",
  logs: "Server log events: recent ones kept in memory, and earlier days from the log files. What is recorded is set under Settings › Logging.",
  jobs: "Background jobs across every workspace, the maintenance that keeps the server tidy, and backups.",
  audit: "What system administrators did, kept durably.",
  console: "A command line to this server for technical administrators: the same actions as these pages, typed.",
};

/** The System area for system administrators. */
export function SystemAdminPanel({ accessToken, onNavigateSection, onOpenWorkspace, section, session, workspaces }: {
  accessToken: string;
  onNavigateSection: (section: SystemSection) => void;
  onOpenWorkspace: (workspaceId: string) => void;
  section: SystemSection;
  session: SharedSession;
  /** Every workspace (system administrators see all), for the log filter. */
  workspaces: SharedWorkspace[];
}) {
  return <section className="shared-page-content rm-system-page">
    <PageHeader description={SECTION_INTRO[section]} title={`System · ${SECTION_LABELS[section].label}`} />
    {section === "overview" ? <OverviewSection accessToken={accessToken} /> : null}
    {section === "usage" ? <UsageSection accessToken={accessToken} onOpenWorkspace={onOpenWorkspace} /> : null}
    {section === "users" ? <UsersSection accessToken={accessToken} session={session} /> : null}
    {section === "settings" ? <SettingsSection accessToken={accessToken} session={session} /> : null}
    {section === "logs" ? <LogsSection accessToken={accessToken} canDownload={session.is_system_admin === true} onOpenSettings={() => onNavigateSection("settings")} workspaces={workspaces} /> : null}
    {section === "jobs" ? <JobsSection accessToken={accessToken} canChange={session.is_system_admin === true} /> : null}
    {section === "audit" ? <AuditSection accessToken={accessToken} canExport={session.is_system_admin === true} /> : null}
    {section === "console" ? <SystemConsole accessToken={accessToken} /> : null}
  </section>;
}
