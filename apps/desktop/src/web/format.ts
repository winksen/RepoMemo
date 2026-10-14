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

export function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function formatMetricDay(value: string) {
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function artifactTypeLabel(type: ArtifactType) {
  if (type === "markdown_doc") return "Markdown";
  if (type === "code_file") return "Code";
  if (type === "note") return "Shared note";
  return type.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

/** One tag per file: the language or format when known, otherwise the artifact type. */
export function artifactKindLabel(artifact: { artifact_type: ArtifactType; language: string | null }) {
  if (artifact.artifact_type === "note") return "Note";
  if (artifact.artifact_type === "repository") return "Git repository";
  return artifact.language && artifact.language !== "Unspecified" ? artifact.language : artifactTypeLabel(artifact.artifact_type);
}

export function isAiActivity(event: WorkspaceActivityEvent) {
  return ["assistant_answered", "ai_overview_generated", "ai_question_answered"].includes(event.action);
}

export function formatActivityTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
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
  return formatActivityTime(value);
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

export function apiMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) return String((error as SharedApiError).message);
  return "The shared API could not be reached. Confirm that the server is running and try again.";
}
