import { type ReactNode, useState } from "react";
import { IconCode as Code, IconFileSpreadsheet as ExcelFile, IconFileText as FileText, IconFileTypePdf as PdfFile, IconFileWord as WordFile, IconGitBranch as RepositoryIcon, IconLoader2 as Loader, IconMail as MailIcon, IconMarkdown as Markdown, IconNote as Note, IconNotebook as OneNoteIcon, IconPhoto as Photo, IconPresentation as Presentation, IconSearch as Search } from "@tabler/icons-react";
import { type ArtifactType, type Citation, type RepoFileAtCommit, type WorkspaceMetricBreakdown } from "../types";
import { Button } from "../components/ui/button";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { showToast } from "../components/ui/toast";
import { getSharedArtifactAtCommit } from "../lib/sharedApi";
import { type FileCategory } from "./constants";
import { apiMessage, citationLines, formatMetricDay } from "./format";

export function fileCategoryIcon(category: FileCategory, size: number) {
  if (category === "repository") return <RepositoryIcon size={size} />;
  if (category === "markdown") return <Markdown size={size} />;
  if (category === "note") return <Note size={size} />;
  if (category === "code") return <Code size={size} />;
  if (category === "image") return <Photo size={size} />;
  if (category === "word") return <WordFile size={size} />;
  if (category === "excel") return <ExcelFile size={size} />;
  if (category === "powerpoint") return <Presentation size={size} />;
  if (category === "pdf") return <PdfFile size={size} />;
  if (category === "onenote") return <OneNoteIcon size={size} />;
  if (category === "email") return <MailIcon size={size} />;
  return <FileText size={size} />;
}

export function LoadingCanvas({ label }: { label: string }) {
  return (
    <main className="shared-auth-canvas">
      <div className="shared-loading" role="status"><Loader className="spin" size={22} /> {label}</div>
    </main>
  );
}

export function MetricBars({ emptyLabel, formatValue = (value: number) => value.toLocaleString(), items, summary, title }: { emptyLabel: string; formatValue?: (value: number) => string; items: WorkspaceMetricBreakdown[]; summary?: string; title: string }) {
  const visibleItems = items.slice(0, 5);
  const maximum = Math.max(...visibleItems.map((item) => item.value), 1);
  return <section className="shared-metric-panel"><div><h3>{title}</h3><span>{summary ?? (visibleItems.length ? `${visibleItems.length} categories` : "No data yet")}</span></div>{visibleItems.length ? <div className="shared-metric-bars">{visibleItems.map((item) => <div className="shared-metric-bar" key={item.label}><span>{item.label}</span><div aria-label={`${item.label}: ${formatValue(item.value)}`} className="shared-metric-bar-track"><i style={{ width: `${(item.value / maximum) * 100}%` }} /></div><strong>{formatValue(item.value)}</strong></div>)}</div> : <p className="shared-metric-empty">{emptyLabel}</p>}</section>;
}

export function MetricTimeline({ items, title = "Activity, last 14 days" }: { items: WorkspaceMetricBreakdown[]; title?: string }) {
  const maximum = Math.max(...items.map((item) => item.value), 1);
  return <section className="shared-metric-panel shared-metric-timeline"><div><h3>{title}</h3><span>{items.reduce((total, item) => total + item.value, 0)} changes</span></div><div className="shared-timeline-bars" aria-label={`${title} activity graph`}>{items.map((item) => <div key={item.label}><span title={`${formatMetricDay(item.label)}: ${item.value} changes`} style={{ height: `${Math.max((item.value / maximum) * 100, item.value ? 8 : 2)}%` }} /><small>{formatMetricDay(item.label)}</small></div>)}</div></section>;
}

export function ContributionCalendar({ items, title, total }: { items: WorkspaceMetricBreakdown[]; title: string; total?: number }) {
  const calendarDays = items.map((item) => ({ ...item, date: new Date(`${item.label}T12:00:00`) })).filter((item) => !Number.isNaN(item.date.getTime()));
  const maximum = Math.max(...calendarDays.map((item) => item.value), 1);
  const startOffset = calendarDays[0]?.date.getDay() ?? 0;
  const contributionTotal = total ?? calendarDays.reduce((sum, item) => sum + item.value, 0);
  const monthMarkers = calendarDays.filter((item, index) => index === 0 || item.date.getDate() === 1);
  const gridPosition = (index: number) => ({ gridColumnStart: Math.floor((startOffset + index) / 7) + 1, gridRowStart: ((startOffset + index) % 7) + 1 });
  const levelFor = (value: number) => value === 0 ? 0 : Math.max(1, Math.min(4, Math.ceil((value / maximum) * 4)));
  return <section className="shared-contribution-calendar">
    <div className="shared-contribution-heading"><div><h3>{contributionTotal.toLocaleString()} contributions in the last year</h3><span>{title}</span></div></div>
    <div className="shared-contribution-scroll">
      <div className="shared-contribution-months" aria-hidden="true">{monthMarkers.map((item) => { const index = calendarDays.indexOf(item); return <span key={item.label} style={{ gridColumnStart: gridPosition(index).gridColumnStart }}>{item.date.toLocaleDateString(undefined, { month: "short" })}</span>; })}</div>
      <div className="shared-contribution-body"><div className="shared-contribution-weekdays" aria-hidden="true"><span>Sun</span><span></span><span>Tue</span><span></span><span>Thu</span><span></span><span>Sat</span></div><div aria-label={`${title}: ${contributionTotal} contributions in the last year`} className="shared-contribution-grid" role="img">{calendarDays.map((item, index) => <span aria-label={`${formatMetricDay(item.label)}: ${item.value} contributions`} data-level={levelFor(item.value)} key={item.label} style={gridPosition(index)} title={`${formatMetricDay(item.label)}: ${item.value} contributions`} />)}</div></div>
      <div className="shared-contribution-footer"><span>Contribution activity is based on saved evidence, memory, indexing, and workspace changes.</span><div aria-hidden="true" className="shared-contribution-legend"><span>Less</span>{[0, 1, 2, 3, 4].map((level) => <i data-level={level} key={level} />)}<span>More</span></div></div>
    </div>
  </section>;
}

export function artifactTypeIcon(type: ArtifactType, size: number) {
  if (type === "markdown_doc") return <Markdown size={size} />;
  if (type === "code_file") return <Code size={size} />;
  if (type === "note") return <Note size={size} />;
  return <FileText size={size} />;
}

/** The sources behind an AI answer, each opening its file. Ask answers cite
 *  their passages as [1], [2]… in this order, so those are numbered. */
export function EvidenceUsed({ accessToken, citations, numbered = false, onOpenArtifact }: { accessToken?: string; citations: Citation[]; numbered?: boolean; onOpenArtifact: (artifactId: string) => void }) {
  return <div className="shared-ai-citations"><strong>Evidence used</strong>{citations.map((citation, index) => <span className="shared-assistant-citation-row" key={`${citation.artifact_id}-${citation.chunk_id ?? "artifact"}-${index}`}>
    <button className="shared-assistant-citation" onClick={() => onOpenArtifact(citation.artifact_id)} title={`Open ${citation.path}`} type="button">{numbered ? `[${index + 1}] ` : ""}{citation.title} · {citation.path}{citationLines(citation)}</button>
    {accessToken ? <ViewAtCommit accessToken={accessToken} citation={citation} /> : null}
  </span>)}</div>;
}

/** For a citation that names a commit of a repository file: a button that opens
 *  the file's text as it was at that commit, labelled as such, with the cited
 *  lines marked. */
export function ViewAtCommit({ accessToken, citation }: { accessToken: string; citation: Pick<Citation, "artifact_id" | "path" | "start_line" | "end_line"> & { commit?: string | null } }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [file, setFile] = useState<RepoFileAtCommit | null>(null);
  if (!citation.commit) return null;
  const commit = citation.commit;

  async function show() {
    setBusy(true);
    try {
      setFile(await getSharedArtifactAtCommit(accessToken, citation.artifact_id, commit));
      setOpen(true);
    } catch (error) {
      showToast("error", apiMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const lines = file ? file.text.split("\n") : [];
  const from = citation.start_line ?? 0;
  const to = citation.end_line ?? from;
  return <>
    <Button className="shared-assistant-citation-commit" disabled={busy} onClick={() => void show()} title={`Show ${citation.path} as it was at commit ${commit.slice(0, 7)}`} type="button" variant="secondary">View at {commit.slice(0, 7)}</Button>
    {open && file ? <Dialog
      className="rm-dialog-wide"
      description={<>As it was at commit <code>{file.commit.slice(0, 7)}</code>: {file.summary} · {file.author_name} · {new Date(file.committed_at).toLocaleDateString()}. {file.current_commit ? <>This is <strong>not the current version</strong>: the workspace holds <code>{file.current_commit.slice(0, 7)}</code>.</> : "This is also the current version."}</>}
      footer={<DialogCancel label="Close" onClick={() => setOpen(false)} />}
      onClose={() => setOpen(false)}
      open
      title={file.path}
    >
      <pre className="shared-commit-text">{lines.map((line, index) => {
        const number = index + 1;
        const cited = from > 0 && number >= from && number <= to;
        return <div key={number}><span className="rm-map-muted">{String(number).padStart(4, " ")}  </span>{cited ? <mark>{line || " "}</mark> : line}</div>;
      })}</pre>
    </Dialog> : null}
  </>;
}

/** Search snippets mark matches with `<mark>` (from SQLite's `snippet()`).
 *  Only those tags become highlights; everything else stays plain text. */
export function renderHighlightedSnippet(snippet: string): ReactNode {
  let depth = 0;
  return snippet.split(/(<mark>|<\/mark>)/).map((part, index) => {
    if (part === "<mark>") { depth += 1; return null; }
    if (part === "</mark>") { depth = Math.max(0, depth - 1); return null; }
    if (!part) return null;
    return depth > 0 ? <mark key={index}>{part}</mark> : part;
  });
}
