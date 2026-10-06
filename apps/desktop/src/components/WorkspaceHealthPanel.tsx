import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import {
  IconCircleDashed as Unconnected,
  IconCode as Code,
  IconCopy as Copy,
  IconFileAlert as FileAlert,
  IconLinkOff as LinkOff,
  IconRefresh as Refresh,
  IconStethoscope as Stethoscope,
  IconVersions as Versions,
} from "@tabler/icons-react";
import { applySharedHealthAction, getSharedWorkspaceHealth, SharedApiError } from "../lib/sharedApi";
import type { HealthAction, HealthDetector, HealthEvidence, HealthFinding, WorkspaceHealth } from "../types";
import { Button } from "./ui/button";
import { showToast } from "./ui/toast";

const DETECTORS: Record<HealthDetector, { label: string; looksFor: string; icon: ReactNode }> = {
  older_version_active: { label: "Older versions", looksFor: "An earlier upload of a file still active next to the latest one.", icon: <Versions size={16} /> },
  duplicate_content: { label: "Duplicates", looksFor: "Files with byte-for-byte identical content.", icon: <Copy size={16} /> },
  removed_symbol_mentioned: { label: "Removed code", looksFor: "Documents naming a function or type the latest version of its file no longer defines.", icon: <Code size={16} /> },
  outdated_evidence_referenced: { label: "Outdated references", looksFor: "Files naming a file marked outdated or superseded.", icon: <LinkOff size={16} /> },
  index_failed: { label: "Indexing failures", looksFor: "Files search cannot see because indexing failed.", icon: <FileAlert size={16} /> },
  unconnected: { label: "Unconnected", looksFor: "Files close in meaning to nothing else and cited by no memory. Information only.", icon: <Unconnected size={16} /> },
};

const ACTION_LABEL: Record<HealthAction, string> = {
  supersede: "Supersede the others",
  needs_review: "Flag for review",
  mark_outdated: "Mark outdated",
  create_task: "Create task",
  dismiss: "Dismiss",
};

const ACTION_DONE: Record<HealthAction, string> = {
  supersede: "Marked the other files superseded.",
  needs_review: "Flagged for review.",
  mark_outdated: "Marked outdated.",
  create_task: "Task created.",
  dismiss: "Finding dismissed.",
};

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "medium" });
}

function lineLabel(start: number | null, end: number | null) {
  if (start === null) return "";
  return end !== null && end > start ? `lines ${start}–${end}` : `line ${start}`;
}

/** The passage's place, naming its file only when the finding has others. */
function evidenceLabel(item: HealthEvidence, finding: HealthFinding) {
  const lines = lineLabel(item.start_line, item.end_line);
  const onlyFile = finding.files.length === 1 && finding.files[0].artifact_id === item.artifact_id;
  if (onlyFile) return lines ? lines.charAt(0).toUpperCase() + lines.slice(1) : "Open passage";
  return lines ? `${item.title} · ${lines}` : item.title;
}

/** Workspace Health: deterministic checks over stored files, with
 *  reversible actions for workspace administrators. */
export function WorkspaceHealthPanel({
  accessToken,
  canAct,
  onOpenArtifact,
  workspaceId,
}: {
  accessToken: string;
  canAct: boolean;
  onOpenArtifact: (artifactId: string) => void;
  workspaceId: string;
}) {
  const [health, setHealth] = useState<WorkspaceHealth | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [pending, setPending] = useState<string | null>(null);

  async function load() {
    setIsRefreshing(true);
    try {
      setHealth(await getSharedWorkspaceHealth(accessToken, workspaceId));
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "Workspace health could not be checked.");
    } finally {
      setIsRefreshing(false);
    }
  }

  useEffect(() => { setHealth(null); void load(); }, [accessToken, workspaceId]);

  async function act(finding: HealthFinding, action: HealthAction, keepArtifactId?: string) {
    setPending(finding.fingerprint);
    try {
      await applySharedHealthAction(accessToken, workspaceId, finding.fingerprint, action, keepArtifactId);
      showToast("success", ACTION_DONE[action]);
    } catch (error) {
      const stale = error instanceof SharedApiError && error.status === 409;
      showToast(stale ? "warning" : "error", error instanceof Error ? error.message : "The action could not be applied.");
    } finally {
      setPending(null);
      await load();
    }
  }

  if (!health) {
    return <section className="rm-health" aria-busy="true"><p className="shared-muted-copy">Checking workspace health…</p></section>;
  }

  const warnings = health.findings.filter((finding) => finding.severity === "warning");
  const info = health.findings.filter((finding) => finding.severity === "info");

  return <section className="rm-health" aria-busy={isRefreshing}>
    <div className="rm-map-toolbar">
      <span>{health.findings.length ? `${health.findings.length} finding${health.findings.length === 1 ? "" : "s"}` : "No findings"} across {health.checked_file_count.toLocaleString()} file{health.checked_file_count === 1 ? "" : "s"} in use</span>
      <Button disabled={isRefreshing} onClick={() => void load()} type="button" variant="secondary"><Refresh size={15} /> Check again</Button>
    </div>
    {!canAct ? <p className="rm-map-notice">You can review findings. Only workspace owners and administrators can act on them.</p> : null}

    <section className="rm-map-section" aria-labelledby="rm-health-findings-title">
      <div className="shared-panel-heading"><div><Stethoscope size={18} /><h2 id="rm-health-findings-title">Needs attention</h2></div></div>
      {warnings.length
        ? <ul className="rm-health-list">{warnings.map((finding) => <FindingCard busy={pending === finding.fingerprint} canAct={canAct} finding={finding} key={finding.fingerprint} onAct={act} onOpenArtifact={onOpenArtifact} />)}</ul>
        : <p className="shared-muted-copy">Nothing needs attention. Findings you act on or dismiss stay hidden until the files change.</p>}
    </section>

    {info.length ? <section className="rm-map-section" aria-labelledby="rm-health-info-title">
      <div className="shared-panel-heading"><div><Unconnected size={18} /><h2 id="rm-health-info-title">For information</h2></div></div>
      <p className="rm-map-intro">A weak signal: a file unlike the rest can be the most valuable one here. Check that it belongs; nothing is suggested for removal.</p>
      <ul className="rm-health-list">{info.map((finding) => <FindingCard busy={pending === finding.fingerprint} canAct={canAct} finding={finding} key={finding.fingerprint} onAct={act} onOpenArtifact={onOpenArtifact} />)}</ul>
    </section> : null}

    <section className="rm-map-section" aria-labelledby="rm-health-checks-title">
      <div className="shared-panel-heading"><div><FileAlert size={18} /><h2 id="rm-health-checks-title">Checks</h2></div></div>
      <p className="rm-map-intro">Every check reads stored facts only: versions, content hashes, indexed code symbols, lifecycle states and index results. No AI provider is called. Acted-on and dismissed counts show which checks are worth keeping.</p>
      <div className="rm-map-coverage">
        <table>
          <thead><tr><th scope="col">Check</th><th scope="col">Looks for</th><th scope="col">Open</th><th scope="col">Acted on</th><th scope="col">Dismissed</th></tr></thead>
          <tbody>{health.detectors.map((stats) => <tr key={stats.detector}>
            <th scope="row">{DETECTORS[stats.detector].label}</th>
            <td className="rm-health-looks-for">{DETECTORS[stats.detector].looksFor}{stats.detector === "unconnected" && !health.similarity_available ? <span className="rm-map-muted"> Needs an embedding provider.</span> : null}</td>
            <td>{stats.open_count}</td>
            <td>{stats.acted_count}</td>
            <td>{stats.dismissed_count}</td>
          </tr>)}</tbody>
        </table>
      </div>
    </section>
  </section>;
}

function FindingCard({
  busy,
  canAct,
  finding,
  onAct,
  onOpenArtifact,
}: {
  busy: boolean;
  canAct: boolean;
  finding: HealthFinding;
  onAct: (finding: HealthFinding, action: HealthAction, keepArtifactId?: string) => Promise<void>;
  onOpenArtifact: (artifactId: string) => void;
}) {
  const [keep, setKeep] = useState(finding.keep_artifact_id ?? "");
  const choosesKeeper = finding.actions.includes("supersede");
  const detector = DETECTORS[finding.detector];
  const showDates = finding.detector === "older_version_active" || finding.detector === "duplicate_content";

  return <li className={`rm-health-card ${finding.severity}`}>
    <div className="rm-health-card-head">
      <span className="rm-health-kind">{detector.icon}{detector.label}</span>
      <h3>{finding.title}</h3>
      <p>{finding.detail}</p>
    </div>

    {choosesKeeper ? <fieldset className="rm-health-files" disabled={!canAct || busy}>
      <legend>Choose the file to keep</legend>
      {finding.files.map((file) => <label key={file.artifact_id}>
        <input checked={keep === file.artifact_id} name={`keep-${finding.fingerprint}`} onChange={() => setKeep(file.artifact_id)} type="radio" />
        <button className="rm-map-link" onClick={() => onOpenArtifact(file.artifact_id)} type="button">{file.title}</button>
        <span className="rm-map-muted">{file.path !== file.title ? `${file.path} · ` : ""}uploaded {formatDate(file.created_at)}</span>
        <span className={`rm-health-fate${keep === file.artifact_id ? " kept" : ""}`}>{keep === file.artifact_id ? "Kept" : "Superseded"}</span>
      </label>)}
    </fieldset> : <ul className="rm-health-files">
      {finding.files.map((file) => <li key={file.artifact_id}>
        <button className="rm-map-link" onClick={() => onOpenArtifact(file.artifact_id)} type="button">{file.title}</button>
        {file.path !== file.title || showDates ? <span className="rm-map-muted">{file.path}</span> : null}
      </li>)}
    </ul>}

    {finding.evidence.length ? <ul className="rm-health-evidence" aria-label="Evidence">
      {finding.evidence.map((item, index) => <li key={`${item.artifact_id}-${index}`}>
        <button className="rm-map-link" onClick={() => onOpenArtifact(item.artifact_id)} type="button">{evidenceLabel(item, finding)}</button>
        <blockquote>{item.excerpt}</blockquote>
      </li>)}
    </ul> : null}

    {canAct ? <div className="rm-health-actions">
      {finding.actions.map((action, index) => <Button
        disabled={busy || (action === "supersede" && !keep)}
        key={action}
        onClick={() => void onAct(finding, action, action === "supersede" ? keep : undefined)}
        type="button"
        variant={index === 0 && action !== "dismiss" ? "main" : "secondary"}
      >{ACTION_LABEL[action]}</Button>)}
    </div> : null}
  </li>;
}
