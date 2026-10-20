import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { IconAdjustments as Adjustments, IconDownload as Download, IconServer as Server } from "@tabler/icons-react";
import { downloadSystemLogFile, getSystemLogFiles, getSystemLogs } from "../../lib/sharedApi";
import type { SharedWorkspace, SystemLogFiles, SystemLogs } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Dropdown } from "../../components/ui/dropdown";
import { EmptyState } from "../../components/ui/empty-state";
import { Field } from "../../components/ui/field";
import { SearchField } from "../../components/ui/search-field";
import { LoadingState } from "../../components/ui/loading-state";
import { Switch } from "../../components/ui/switch";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage, formatBytes } from "../format";
import { Facts, useSystemData } from "./shared";
import { systemKeys } from "../../lib/queryClient";

const LIVE_SOURCE = "live";
const LEVEL_LABELS: Record<string, string> = { off: "off", error: "errors", warn: "warnings", info: "information", debug: "debug", trace: "trace" };

export function LogsSection({ accessToken, canDownload, onOpenSettings, workspaces }: { accessToken: string; canDownload: boolean; onOpenSettings: () => void; workspaces: SharedWorkspace[] }) {
  const [source, setSource] = useState(LIVE_SOURCE);
  const [level, setLevel] = useState("info");
  const [category, setCategory] = useState("all");
  const [workspace, setWorkspace] = useState("all");
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
  const isLive = source === LIVE_SOURCE;
  // The search is sent a moment after the last key, not on every key.
  const [searched, setSearched] = useState("");
  useEffect(() => {
    const timer = window.setTimeout(() => setSearched(query.trim()), query ? 300 : 0);
    return () => window.clearTimeout(timer);
  }, [query]);
  const filter = {
    level,
    category: category === "all" ? undefined : category,
    workspace: workspace === "all" ? undefined : workspace,
    day: isLive ? undefined : source,
    q: searched || undefined,
  };
  const [files] = useSystemData<SystemLogFiles>(systemKeys.logFiles(), () => getSystemLogFiles(accessToken), { failure: "The log files could not be listed." });
  const [logs] = useSystemData<SystemLogs>(systemKeys.logs(filter), () => getSystemLogs(accessToken, filter), { failure: "Logs could not be loaded.", keepPrevious: true, refreshMs: follow && isLive ? 5_000 : undefined });

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
      <Field label="Source">{(control) => <Dropdown id={control.id} onValueChange={setSource} options={sourceOptions} value={source} />}</Field>
      <Field label="Category">{(control) => <Dropdown id={control.id} onValueChange={setCategory} options={categoryOptions} value={category} />}</Field>
      <Field label="Workspace">{(control) => <Dropdown id={control.id} onValueChange={setWorkspace} options={workspaceOptions} value={workspace} />}</Field>
      <Field label="Level">{(control) => <Dropdown id={control.id} onValueChange={setLevel} options={[{ label: "Errors", value: "error" }, { label: "Warnings and up", value: "warn" }, { label: "Information and up", value: "info" }, { label: "Everything kept", value: "trace" }]} value={level} />}</Field>
    </div>
    <div className="rm-system-toolbar">
      <SearchField aria-label="Search logs" onValueChange={setQuery} placeholder="Search message, source or fields" value={query} />
      <div className="rm-system-actions">
        {isLive ? <Switch checked={follow} label="Follow" onCheckedChange={setFollow} /> : canDownload ? <Button onClick={() => void download()} variant="secondary"><Download size={ICON_SIZE.sm} /> Download {source}</Button> : null}
      </div>
    </div>
    {logs && !logs.capturing ? <p className="shared-muted-copy">This server is not capturing logs in memory; read them from its console or log collector.</p> : null}
    {logs?.records.length ? <ol className="rm-system-log" aria-label="Log events, newest first">{logs.records.map((record) => <li className={`level-${record.level}`} key={`${record.sequence}-${record.timestamp}`}>
      <time dateTime={record.timestamp}>{isLive ? new Date(record.timestamp).toLocaleTimeString() : new Date(record.timestamp).toLocaleString()}</time>
      <span className="rm-system-log-level">{record.level}</span>
      <span className="rm-system-log-target">{record.target}</span>
      <span className="rm-system-log-message">{record.message}{record.fields ? <small>{record.fields}</small> : null}</span>
    </li>)}</ol> : logs ? <EmptyState title="No log events match" /> : <LoadingState label="Loading logs" />}
  </>;
}
