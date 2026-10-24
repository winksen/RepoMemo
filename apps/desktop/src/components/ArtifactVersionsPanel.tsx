import { useEffect, useState } from "react";
import { IconHistory as History, IconLoader2 as Loader } from "@tabler/icons-react";
import { compareSharedArtifactVersions, getSharedArtifactVersionText, listSharedArtifactVersions, restoreSharedArtifactVersion } from "../lib/sharedApi";
import type { ArtifactVersion, ArtifactVersionText, VersionComparison } from "../types";
import { apiMessage, formatBytes } from "../web/format";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { Dialog, DialogCancel } from "./ui/dialog";
import { useConfirm } from "./ui/confirm-dialog";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";

type Opened =
  | { kind: "text"; version: number; text: ArtifactVersionText | null }
  | { kind: "compare"; from: number; to: number; comparison: VersionComparison | null };

function when(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/**
 * The versions of an uploaded file. Uploading a new version keeps the one it
 * replaces: each stays readable here, and any two can be compared line by
 * line. Shown only once a file has more than one. A person who can change the file can restore an earlier
 * version (V0.3.8): it becomes a new version with that content, and the version it replaces stays in the list.
 */
export function ArtifactVersionsPanel({ accessToken, artifactId, canWrite = false, onRestored, updatedAt }: { accessToken: string; artifactId: string; canWrite?: boolean; onRestored?: () => void; updatedAt: string }) {
  const [versions, setVersions] = useState<ArtifactVersion[] | null>(null);
  const [opened, setOpened] = useState<Opened | null>(null);
  const [restoring, setRestoring] = useState<number | null>(null);
  const confirm = useConfirm();

  // Read again whenever the file changes (a new version was uploaded).
  useEffect(() => {
    let cancelled = false;
    listSharedArtifactVersions(accessToken, artifactId)
      .then((next) => { if (!cancelled) setVersions(next); })
      .catch(() => { if (!cancelled) setVersions(null); });
    return () => { cancelled = true; };
  }, [accessToken, artifactId, updatedAt]);

  async function viewText(version: number) {
    setOpened({ kind: "text", version, text: null });
    try {
      const text = await getSharedArtifactVersionText(accessToken, artifactId, version);
      setOpened((current) => current?.kind === "text" && current.version === version ? { kind: "text", version, text } : current);
    } catch (error) {
      setOpened(null);
      showToast("error", apiMessage(error, "That version could not be read."));
    }
  }

  async function restore(version: number) {
    const next = (versions?.[0]?.version ?? version) + 1;
    const agreed = await confirm({
      action: "Restore",
      description: `Version ${version} becomes version ${next}, the new current version. The version it replaces stays in the history, and the file is indexed again.`,
      title: `Restore version ${version}?`,
    });
    if (!agreed) return;
    setRestoring(version);
    try {
      await restoreSharedArtifactVersion(accessToken, artifactId, version);
      showToast("success", `Version ${version} restored as version ${next}. It is being indexed again.`);
      onRestored?.();
    } catch (error) {
      showToast("error", apiMessage(error, "That version could not be restored."));
    } finally {
      setRestoring(null);
    }
  }

  async function compare(from: number, to: number) {
    setOpened({ kind: "compare", from, to, comparison: null });
    try {
      const comparison = await compareSharedArtifactVersions(accessToken, artifactId, from, to);
      setOpened((current) => current?.kind === "compare" && current.from === from && current.to === to ? { kind: "compare", from, to, comparison } : current);
    } catch (error) {
      setOpened(null);
      showToast("error", apiMessage(error, "The versions could not be compared."));
    }
  }

  if (!versions || versions.length < 2) return null;
  const current = versions[0];

  let dialog = null;
  if (opened?.kind === "text") {
    const text = opened.text;
    dialog = <Dialog size="lg" description={text ? undefined : "Reading this version…"} footer={<DialogCancel label="Close" onClick={() => setOpened(null)} />} onClose={() => setOpened(null)} open title={`Version ${opened.version}`}>
      {text === null ? <p className="shared-muted-copy"><Loader className="spin" size={ICON_SIZE.sm} /> Loading…</p>
        : text.text !== null ? <><pre className="shared-version-text">{text.text}</pre>{text.truncated ? <p className="shared-muted-copy">Only the start of this version is shown.</p> : null}</>
          : <p className="shared-muted-copy">{text.note ?? "This version has no text to show."}</p>}
    </Dialog>;
  } else if (opened?.kind === "compare") {
    const comparison = opened.comparison;
    dialog = <Dialog size="lg" description={comparison ? `${comparison.removed} ${comparison.removed === 1 ? "line" : "lines"} removed, ${comparison.added} added` : "Comparing…"} footer={<DialogCancel label="Close" onClick={() => setOpened(null)} />} onClose={() => setOpened(null)} open title={`Version ${opened.from} → version ${opened.to}`}>
      {comparison === null ? <p className="shared-muted-copy"><Loader className="spin" size={ICON_SIZE.sm} /> Loading…</p>
        : comparison.note && !comparison.lines.length ? <p className="shared-muted-copy">{comparison.note}</p>
          : <>
            {comparison.note ? <p className="shared-muted-copy">{comparison.note}</p> : null}
            <div aria-label="Changes between the two versions" className="shared-version-diff" role="table">
              {comparison.lines.map((line, index) => <div className={`shared-diff-line ${line.kind}`} key={index} role="row">
                <span aria-hidden="true" className="shared-diff-number">{line.old_line ?? ""}</span>
                <span aria-hidden="true" className="shared-diff-number">{line.new_line ?? ""}</span>
                <span aria-hidden="true" className="shared-diff-mark">{line.kind === "added" ? "+" : line.kind === "removed" ? "−" : ""}</span>
                <code>{line.kind === "gap" ? `… ${line.text}` : line.text || " "}</code>
                {line.kind === "added" ? <span className="sr-only">added</span> : line.kind === "removed" ? <span className="sr-only">removed</span> : null}
              </div>)}
            </div>
            {comparison.truncated ? <p className="shared-muted-copy">The files are long: only their first lines were compared.</p> : null}
          </>}
    </Dialog>;
  }

  return <section className="shared-record-panel shared-versions-panel" aria-label="Version history">
    {dialog}
    <PanelHeading description={<>{versions.length} versions</>} icon={History} title="Version history" />
    <ul className="shared-version-list">
      {versions.map((version) => <li key={version.version}>
        <div>
          <strong>Version {version.version}{version.is_current ? " · current" : ""}</strong>
          <small>{version.uploaded_by ? `Uploaded by ${version.uploaded_by} · ` : version.version === 1 ? "First upload · " : ""}{when(version.created_at)} · {formatBytes(version.size_bytes)}</small>
        </div>
        <div className="shared-version-actions">
          <Button onClick={() => void viewText(version.version)} type="button" variant="secondary">View text</Button>
          {version.version > 1 ? <Button onClick={() => void compare(version.version - 1, version.version)} type="button" variant="secondary">What changed</Button> : null}
          {!version.is_current ? <Button onClick={() => void compare(version.version, current.version)} type="button" variant="secondary">Compare with current</Button> : null}
          {canWrite && !version.is_current ? <Button aria-label={`Restore version ${version.version}`} busy={restoring === version.version} disabled={restoring !== null} onClick={() => void restore(version.version)} type="button" variant="secondary">Restore</Button> : null}
        </div>
      </li>)}
    </ul>
  </section>;
}
