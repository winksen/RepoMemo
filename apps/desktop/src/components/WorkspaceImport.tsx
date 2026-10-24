import { type ChangeEvent, useRef, useState } from "react";
import { IconPackageImport as PackageImport } from "@tabler/icons-react";
import { importSharedWorkspaceArchive } from "../lib/sharedApi";
import type { ArchiveImportReport, SharedWorkspace } from "../types";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";
import { apiMessage, formatBytes } from "../web/format";

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
    // Archives of format 2 (V0.3.19) also carry earlier file versions and saved searches; older ones do not say.
    ...(counts.versions ? [part(counts.versions, "earlier version", "earlier versions")] : []),
    ...(counts.saved_searches ? [part(counts.saved_searches, "saved search", "saved searches")] : []),
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
        reset();
      }
    } catch (error) {
      showToast("error", apiMessage(error, "The archive could not be imported."));
    } finally { setIsBusy(false); }
  }

  return (
    <section className="shared-settings-group shared-import-workspace">
      <PanelHeading description="from an archive" icon={PackageImport} title="Import a workspace" />
      <p className="shared-muted-copy">Bring in a workspace exported from RepoMemo as a new workspace. The archive is checked first and nothing is written until you confirm. Comments, tasks and history are credited to you, with the original names kept in the text.</p>
      <div className="shared-import-row">
        <input accept=".gz,.tgz,application/gzip" aria-label="Workspace archive" disabled={isBusy} onChange={choose} ref={input} type="file" />
        <Button disabled={!file || isBusy} onClick={() => void run(true)} type="button" variant="secondary">{isBusy && !report ? "Checking…" : "Check the archive"}</Button>
      </div>
      {report ? <div className="shared-import-report" role="status">
        <strong>{report.workspace_name}</strong>
        <span>{describeArchive(report)} · {formatBytes(report.total_bytes)} · exported {new Date(report.exported_at).toLocaleDateString()} by RepoMemo {report.source_version}</span>
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
