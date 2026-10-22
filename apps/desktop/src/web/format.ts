import { IconCode as Code, IconMarkdown as Markdown, IconNote as Note } from "@tabler/icons-react";
import { SharedApiError } from "../lib/sharedApi";
import { type ArtifactSummary, type ArtifactType, type Citation, type CollaborationTask, type WorkspaceActivityEvent } from "../types";
import { DOCUMENT_KIND_LABEL, documentKindOf } from "../lib/documents";
import { type FileCategory } from "./constants";

/** Maps an artifact to the supported file categories shown in the type filter. */
export function fileCategory(artifact: Pick<ArtifactSummary, "artifact_type" | "language" | "path">): FileCategory {
  if (artifact.artifact_type === "repository") return "repository";
  const documentKind = documentKindOf(artifact);
  if (documentKind) return documentKind;
  if (artifact.artifact_type === "image") return "image";
  if (artifact.artifact_type === "code_file") return "code";
  if (artifact.artifact_type === "note") return "note";
  if (artifact.artifact_type === "markdown_doc") return "markdown";
  return "text";
}

export function documentKindLabel(artifact: Pick<ArtifactSummary, "path">) {
  const kind = documentKindOf(artifact);
  return kind ? DOCUMENT_KIND_LABEL[kind] : "Document";
}

function parseDate(value: string): Date | null {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

/** "3 Mar 2026". An empty value gives `fallback`; a value that is not a date is shown as it is. */
export function formatDate(value: string | null | undefined, fallback = "—") {
  if (!value) return fallback;
  const date = parseDate(value);
  return date ? date.toLocaleDateString(undefined, { dateStyle: "medium" }) : value;
}

/** "3 Mar 2026, 14:05": a date with its time. Empty and invalid values are handled as in `formatDate`. */
export function formatTime(value: string | null | undefined, fallback = "—") {
  if (!value) return fallback;
  const date = parseDate(value);
  return date ? date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : value;
}

/** "5m ago", "3h ago", "2d ago", then the date: how long ago something happened, for lists. */
export function formatRelativeTime(value: string, now: number = Date.now()) {
  const date = parseDate(value);
  if (!date) return value;
  const elapsed = now - date.getTime();
  if (elapsed < 0) return formatDate(value);
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return date.toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

/** The time of day for today, the day and month for an earlier date; empty when it is not a date. */
export function formatRecentTime(value: string) {
  const date = parseDate(value);
  if (!date) return "";
  return date.toDateString() === new Date().toDateString()
    ? date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })
    : date.toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

/** "512 B", "48 KB", "1.5 MB": a size, with no decimals below a megabyte. */
export function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes < 1024 * 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  return `${(bytes / (1024 * 1024 * 1024 * 1024)).toFixed(1)} TB`;
}

/** "3 d 4 h", "5 h 10 min", "12 min": how long a server has been up. */
export function formatDuration(seconds: number) {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days) return `${days} d ${hours} h`;
  if (hours) return `${hours} h ${minutes} min`;
  return `${minutes} min`;
}

/** "42%", or "<1%" for a share too small to round. */
export function formatShare(share: number) {
  if (share > 0 && share < 0.01) return "<1%";
  return `${Math.round(share * 100)}%`;
}

export function formatMetricDay(value: string) {
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/** The name of a kind of evidence; `labels` are the names the workspace's template gives (V0.2.173). */
export function artifactTypeLabel(type: ArtifactType, labels?: Record<string, string> | null) {
  if (labels?.[type]) return labels[type];
  if (type === "markdown_doc") return "Markdown";
  if (type === "code_file") return "Code";
  if (type === "note") return "Shared note";
  return type.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

/** One tag per file: the language or format when known, otherwise the artifact type. */
export function artifactKindLabel(artifact: { artifact_type: ArtifactType; language: string | null }, labels?: Record<string, string> | null) {
  if (artifact.artifact_type === "note") return labels?.note ?? "Note";
  if (artifact.artifact_type === "repository") return labels?.repository ?? "Git repository";
  return artifact.language && artifact.language !== "Unspecified" ? artifact.language : artifactTypeLabel(artifact.artifact_type, labels);
}

export function isAiActivity(event: WorkspaceActivityEvent) {
  return ["assistant_answered", "ai_overview_generated", "ai_question_answered"].includes(event.action);
}

export function isTaskOverdue(task: CollaborationTask) {
  if (!task.due_at || task.status === "done") return false;
  const dueAt = new Date(task.due_at);
  return !Number.isNaN(dueAt.getTime()) && dueAt.getTime() < Date.now();
}

export function formatTaskDueDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function formatProfileTime(value: string | null | undefined) {
  if (!value) return "Not recorded yet";
  return formatTime(value);
}

export function artifactCitation(artifacts: ArtifactSummary[], artifactId: string) {
  const artifact = artifacts.find((entry) => entry.id === artifactId);
  if (!artifact) return [];
  return [{
    artifact_id: artifact.id,
    chunk_id: null,
    title: artifact.title,
    path: artifact.path,
    start_line: null,
    end_line: null,
    confidence: null,
  }];
}

/** Where in the file a passage is, and for a repository file the commit its text is from. */
export function citationLines(citation: Pick<Citation, "start_line" | "end_line"> & { commit?: string | null }) {
  const commit = citation.commit ? ` · commit ${citation.commit.slice(0, 7)}` : "";
  if (!citation.start_line) return commit;
  const lines = citation.end_line && citation.end_line !== citation.start_line ? ` · lines ${citation.start_line}–${citation.end_line}` : ` · line ${citation.start_line}`;
  return `${lines}${commit}`;
}

const UNREACHABLE = "The shared API could not be reached. Confirm that the server is running and try again.";

/** The message of an error, for a toast; `fallback` when it has none (a thrown string, an empty message). */
export function apiMessage(error: unknown, fallback: string = UNREACHABLE): string {
  if (error && typeof error === "object" && "message" in error) {
    const message = String((error as SharedApiError).message);
    if (message) return message;
  }
  return fallback;
}

/** "Ana · 3 Mar 2026": who last changed a repository file, and when. */
export function lastChangeLabel(author: string | null | undefined, at: string | null | undefined): string {
  if (!author) return "";
  const date = at ? new Date(at) : null;
  const when = date && !Number.isNaN(date.getTime()) ? date.toLocaleDateString(undefined, { dateStyle: "medium" }) : "";
  return when ? `${author} · ${when}` : author;
}
