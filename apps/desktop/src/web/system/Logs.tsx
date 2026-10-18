import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { IconAdjustments as Adjustments, IconDownload as Download, IconServer as Server } from "@tabler/icons-react";
import { downloadSystemLogFile, getSystemLogFiles, getSystemLogs } from "../../lib/sharedApi";
import type { SharedWorkspace, SystemLogFiles, SystemLogs } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Dropdown } from "../../components/ui/dropdown";
import { SearchField } from "../../components/ui/search-field";
import { LoadingState } from "../../components/ui/loading-state";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage, formatBytes } from "../format";
import { Facts } from "./shared";

const LIVE_SOURCE = "live";
const LEVEL_LABELS: Record<string, string> = { off: "off", error: "errors", warn: "warnings", info: "information", debug: "debug", trace: "trace" };

export function LogsSection({ accessToken, canDownload, onOpenSettings, workspaces }: { accessToken: string; canDownload: boolean; onOpenSettings: () => void; workspaces: SharedWorkspace[] }) {
  const [source, setSource] = useState(LIVE_SOURCE);
  const [level, setLevel] = useState("info");
  const [category, setCategory] = useState("all");
  const [workspace, setWorkspace] = useState("all");
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
  const [logs, setLogs] = useState<SystemLogs | null>(null);
  const [files, setFiles] = useState<SystemLogFiles | null>(null);
  const isLive = source === LIVE_SOURCE;

  useEffect(() => {
    getSystemLogFiles(accessToken).then(setFiles).catch((error) => showToast("error", apiMessage(error, "The log files could not be listed.")));
  }, [accessToken]);

  useEffect(() => {
    let active = true;
    const load = () => getSystemLogs(accessToken, {
      level,
      category: category === "all" ? undefined : category,
      workspace: workspace === "all" ? undefined : workspace,
      day: isLive ? undefined : source,
      q: query.trim() || undefined,
    })
      .then((next) => { if (active) setLogs(next); })
      .catch((error) => { if (active) showToast("error", apiMessage(error, "Logs could not be loaded.")); });
    const delay = window.setTimeout(() => void load(), query ? 300 : 0);
    const timer = follow && isLive ? window.setInterval(() => void load(), 5_000) : undefined;
    return () => { active = false; window.clearTimeout(delay); if (timer) window.clearInterval(timer); };
  }, [accessToken, level, category, workspace, source, query, follow, isLive]);

  async function download() {
    try {
      const blob = await downloadSystemLogFile(accessToken, source);
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `repomemo-${source}.jsonl`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 10_000);
    } catch (error) { showToast("error", apiMessage(error, "The log file could not be downloaded.")); }
  }

  const sourceOptions = [{ label: "Live: recent events", value: LIVE_SOURCE }, ...(files?.files ?? []).map((file) => ({ label: `${file.day} · ${formatBytes(file.bytes)}`, value: file.day }))];
  const categoryOptions = [{ label: "All sources", value: "all" }, ...(files?.categories ?? []).map((entry) => ({ label: entry.label, value: entry.key })), { label: "Server and other", value: "server" }];
  const workspaceOptions = [{ label: "All workspaces", value: "all" }, ...workspaces.map((entry) => ({ label: entry.workspace.name, value: entry.workspace.id }))];
  return <>
    {files ? <section className="shared-settings-group">
      <PanelHeading actions={<Button onClick={onOpenSettings} type="button" variant="secondary"><Adjustments size={ICON_SIZE.sm} /> Logging settings</Button>} icon={Adjustments} title="What is recorded" />
      {files.environment_filter
        ? <p className="shared-muted-copy">The server's RUST_LOG filter is in force: <code>{files.effective_filter}</code>. Choosing levels in Settings replaces it.</p>
        : <Facts items={[
          ["Server and other", LEVEL_LABELS[files.default_level] ?? files.default_level],
          ...files.categories.map((entry): [string, ReactNode] => [entry.label, LEVEL_LABELS[entry.level] ?? entry.level]),
        ]} />}
      <p className="shared-muted-copy">Console: {files.console_format === "json" ? "JSON lines" : `${files.console_format} text`} · Files: {files.writing_files ? `daily, kept ${files.retention_days ? `${files.retention_days} days` : "forever"}` : "off"}</p>
    </section> : null}
    <div className="rm-system-toolbar">
      <label className="rm-system-inline">Source<Dropdown aria-label="Log source" onValueChange={setSource} options={sourceOptions} value={source} /></label>
      <label className="rm-system-inline">Category<Dropdown aria-label="Log category" onValueChange={setCategory} options={categoryOptions} value={category} /></label>
      <label className="rm-system-inline">Workspace<Dropdown aria-label="Workspace" onValueChange={setWorkspace} options={workspaceOptions} value={workspace} /></label>
      <label className="rm-system-inline">Level<Dropdown aria-label="Lowest level shown" onValueChange={setLevel} options={[{ label: "Errors", value: "error" }, { label: "Warnings and up", value: "warn" }, { label: "Information and up", value: "info" }, { label: "Everything kept", value: "trace" }]} value={level} /></label>
    </div>
    <div className="rm-system-toolbar">
      <SearchField aria-label="Search logs" onValueChange={setQuery} placeholder="Search message, source or fields" value={query} />
      <div className="rm-system-actions">
        {isLive ? <label className="shared-ai-provider-toggle"><input checked={follow} onChange={(event) => setFollow(event.target.checked)} type="checkbox" /> Follow</label> : canDownload ? <Button onClick={() => void download()} type="button" variant="secondary"><Download size={ICON_SIZE.sm} /> Download {source}</Button> : null}
      </div>
    </div>
    {logs && !logs.capturing ? <p className="shared-muted-copy">This server is not capturing logs in memory; read them from its console or log collector.</p> : null}
    {logs?.records.length ? <ol className="rm-system-log" aria-label="Log events, newest first">{logs.records.map((record) => <li className={`level-${record.level}`} key={`${record.sequence}-${record.timestamp}`}>
      <time dateTime={record.timestamp}>{isLive ? new Date(record.timestamp).toLocaleTimeString() : new Date(record.timestamp).toLocaleString()}</time>
      <span className="rm-system-log-level">{record.level}</span>
      <span className="rm-system-log-target">{record.target}</span>
      <span className="rm-system-log-message">{record.message}{record.fields ? <small>{record.fields}</small> : null}</span>
    </li>)}</ol> : logs ? <p className="shared-muted-copy">No log events match.</p> : <LoadingState label="Loading logs" />}
  </>;
}
