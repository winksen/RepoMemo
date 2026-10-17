import { type ChangeEvent, useEffect, useRef, useState } from "react";
import { IconPackageImport as PackageImport } from "@tabler/icons-react";
import { importSharedWorkspaceArchive } from "../lib/sharedApi";
import { isDesktopRuntime } from "../lib/desktop";
import { discardPreparedArchive, loadPreparedArchive, pendingHandoff, type PreparedArchive, rememberHandoff } from "../lib/handoff";
import type { ArchiveImportReport, SharedWorkspace } from "../types";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} bytes`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${value.toFixed(1)} ${units[unit]}`;
}

/** What the archive holds, as a sentence. */
export function describeArchive(report: ArchiveImportReport): string {
  const { counts } = report;
  const part = (count: number, one: string, many: string) => `${count.toLocaleString()} ${count === 1 ? one : many}`;
  return [
    part(counts.artifacts, "file", "files"),
    part(counts.folders, "folder", "folders"),
    part(counts.memory_cards, "memory card", "memory cards"),
    part(counts.tasks, "task", "tasks"),
    part(counts.comments, "comment", "comments"),
  ].join(", ");
}

/**
 * Brings a workspace archive into a new workspace of the organization. The
 * file is checked first (nothing is written), and imported only when the
 * person has seen what is in it.
 */
export function WorkspaceImport({ accessToken, onImported, organizationId }: {
  accessToken: string;
  onImported: (workspace: SharedWorkspace) => void;
  organizationId: string;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [file, setFile] = useState<File | null>(null);
  const [name, setName] = useState("");
  const [report, setReport] = useState<ArchiveImportReport | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  // In the desktop app: a local workspace prepared as an archive, waiting for a server (V0.2.50). It is checked as soon as this page shows.
  const [handoff, setHandoff] = useState<PreparedArchive | null>(() => (isDesktopRuntime ? pendingHandoff() : null));

  useEffect(() => {
    if (!handoff) return;
    let cancelled = false;
    (async () => {
      setIsBusy(true);
      try {
        const prepared = await loadPreparedArchive(handoff);
        if (cancelled) return;
        setFile(prepared);
        const answer = await importSharedWorkspaceArchive(accessToken, { organizationId, archive: prepared, dryRun: true, name: "" });
        if (cancelled) return;
        setReport(answer);
        setName(answer.workspace_name);
      } catch (error) {
        if (!cancelled) showToast("error", typeof error === "string" ? error : error instanceof Error ? error.message : "The prepared archive could not be read.");
      } finally { if (!cancelled) setIsBusy(false); }
    })();
    return () => { cancelled = true; };
  }, [handoff, accessToken, organizationId]);

  async function forgetHandoff() {
    if (handoff) await discardPreparedArchive(handoff).catch(() => undefined);
    rememberHandoff(null);
    setHandoff(null);
    reset();
  }

  function choose(event: ChangeEvent<HTMLInputElement>) {
    setFile(event.target.files?.[0] ?? null);
    setReport(null);
    setName("");
  }

  function reset() {
    setFile(null); setReport(null); setName("");
    if (input.current) input.current.value = "";
  }

  async function run(dryRun: boolean) {
    if (!file) return;
    setIsBusy(true);
    try {
      const answer = await importSharedWorkspaceArchive(accessToken, { organizationId, archive: file, dryRun, name });
      if (dryRun) {
        setReport(answer);
        if (!name) setName(answer.workspace_name);
      } else if (answer.workspace) {
        showToast("success", `Imported ${answer.workspace.workspace.name}.`);
        onImported(answer.workspace);
        if (handoff) {
          // The archive has done its work: it is not kept on this computer.
          await discardPreparedArchive(handoff).catch(() => undefined);
          rememberHandoff(null);
          setHandoff(null);
        }
        reset();
      }
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "The archive could not be imported.");
    } finally { setIsBusy(false); }
  }

  return (
    <section className="shared-settings-group shared-import-workspace">
      <div className="shared-panel-heading"><div><PackageImport size={ICON_SIZE.lg} /><h2>Import a workspace</h2></div><span>from an archive</span></div>
      <p className="shared-muted-copy">Bring in a workspace exported from RepoMemo as a new workspace. The archive is checked first and nothing is written until you confirm. Comments, tasks and history are credited to you, with the original names kept in the text.</p>
      {handoff ? <p className="shared-import-handoff" role="status">Prepared on this computer: <strong>{handoff.workspace_name}</strong>. It is being checked below; nothing is written until you import it. <Button onClick={() => void forgetHandoff()} type="button" variant="secondary">Discard it</Button></p> : null}
      <div className="shared-import-row">
        <input accept=".gz,.tgz,application/gzip" aria-label="Workspace archive" disabled={isBusy} onChange={choose} ref={input} type="file" />
        <Button disabled={!file || isBusy} onClick={() => void run(true)} type="button" variant="secondary">{isBusy && !report ? "Checking…" : "Check the archive"}</Button>
      </div>
      {report ? <div className="shared-import-report" role="status">
        <strong>{report.workspace_name}</strong>
        <span>{describeArchive(report)} · {formatSize(report.total_bytes)} · exported {new Date(report.exported_at).toLocaleDateString()} by RepoMemo {report.source_version}</span>
        {report.problems.length ? <ul aria-label="Problems">{report.problems.map((problem) => <li key={problem}>{problem}</li>)}</ul> : null}
        {report.warnings.length ? <ul aria-label="Warnings">{report.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul> : null}
        {report.ok ? <>
          <label>Name of the new workspace<Input onChange={(event) => setName(event.target.value)} value={name} /></label>
          <div><Button disabled={isBusy || !name.trim()} onClick={() => void run(false)} type="button" variant="main">{isBusy ? "Importing…" : "Import workspace"}</Button></div>
        </> : <span>This archive cannot be imported.</span>}
      </div> : null}
    </section>
  );
}
