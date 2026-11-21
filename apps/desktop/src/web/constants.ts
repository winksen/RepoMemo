import { IconCode as Code, IconMarkdown as Markdown, IconNote as Note } from "@tabler/icons-react";
import { type ArtifactSummary, type Citation, type CollaborationTaskStatus, type Folder } from "../types";

export const SYSTEM_SECTIONS = ["overview", "usage", "users", "settings", "logs", "jobs", "audit", "console"] as const;
export type SystemSection = typeof SYSTEM_SECTIONS[number];

/** What the Retrieval page can limit a search to, by evidence state. */
export type RetrievalState = "current" | "verified" | "needs_review" | "retired" | "all";
export const RETRIEVAL_STATES: Record<RetrievalState, { label: string; statuses: string[] }> = {
  current: { label: "Current evidence", statuses: [] },
  verified: { label: "Verified only", statuses: ["verified"] },
  needs_review: { label: "Needing review", statuses: ["needs_review"] },
  retired: { label: "Outdated and superseded", statuses: ["outdated", "superseded"] },
  all: { label: "Every state", statuses: ["active", "needs_review", "verified", "outdated", "superseded"] },
};

export const MAX_FOLDER_DEPTH = 5;

export const INDEX_POLL_INTERVAL_MS = 3000;

// Covers the server's retry schedule (30 s, 2 min, 10 min) before showing a generic problem.
export const INDEX_POLL_ATTEMPTS = 300;

/** With the live event stream open, polling is only a safety net. */
export const LIVE_FALLBACK_POLL_MS = 30_000;

/** Bursts of events (a folder upload, an embedding pass) coalesce into one reload. */
export const LIVE_REFRESH_DELAY_MS = 600;

export const LIVE_METRICS_DELAY_MS = 3_000;

/** Activity that changes what the evidence ledger shows. */
export const LEDGER_ACTIONS = new Set([
  "evidence_stored", "evidence_uploaded", "evidence_moved", "artifact_updated", "artifact_deleted", "artifact_indexed",
  "folder_created", "folder_renamed", "folder_deleted", "folder_moved", "evidence_lifecycle_updated", "repository_connected", "repository_removed", "repository_synced",
]);

export const MEMORY_ACTIONS = new Set(["memory_created", "memory_updated", "memory_deleted"]);

export const TASK_ACTIONS = new Set(["task_created", "task_updated", "task_deleted"]);

export const MAX_UPLOAD_BYTES = 10 * 1024 * 1024;

/** Uploaded at once when several files are chosen. */
export const UPLOAD_CONCURRENCY = 3;

/** File types the shared upload accepts, besides business documents. */
export const UPLOAD_EXTENSIONS = ["md", "mdx", "txt", "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cs", "php", "json", "toml", "yaml", "yml", "sql", "html", "css", "sh", "ps1", "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

/** Folders skipped when a whole folder is uploaded, as in desktop folder import. */
export const SKIPPED_UPLOAD_FOLDERS = new Set([".git", "node_modules", "target", "dist", "build", ".next", ".vite"]);

export const JOB_KIND_LABEL: Record<string, string> = {
  embedding: "Making passages searchable by meaning",
  repo_sync: "Syncing a repository",
};

export const JOB_STAGE_LABEL: Record<string, string> = {
  embedding_chunks: "Embedding passages",
  queued: "Waiting for another sync to finish",
  reading_repository: "Reading the repository",
  storing_files: "Storing changed files",
  indexing: "Indexing",
};

export type LiveRefresh = { timer: number | null; full: boolean; ledger: boolean; memory: boolean; tasks: boolean; metrics: boolean };

export const NO_LIVE_REFRESH: LiveRefresh = { timer: null, full: false, ledger: false, memory: false, tasks: false, metrics: false };

export type UploadProgress = { done: number; total: number; failed: number };

export type MemoryDraft = { title: string; body: string; source: string; citations: Citation[]; selected: Set<number> };

export type AuthMode = "sign-in" | "sign-up";

export type PageState = "restoring" | "unauthenticated" | "ready" | "error";

export type WorkspaceSection = "overview" | "evidence" | "documents" | "repositories" | "retrieval" | "map" | "health" | "review" | "memory" | "tasks" | "people" | "settings";

export type ArtifactViewMode = "grid" | "list";

export type OrganizationNavigation = {
  activeOrganizationId: string | null;
  isCreatingOrganization: boolean;
  selectOrganization: (organizationId: string) => void;
  startCreatingOrganization: () => void;
  cancelOrganizationCreation: () => void;
};

export const WORKSPACE_SECTIONS: WorkspaceSection[] = ["overview", "evidence", "documents", "repositories", "retrieval", "map", "health", "review", "memory", "tasks", "people", "settings"];

export const ADMIN_WORKSPACE_SECTIONS = new Set<WorkspaceSection>(["people", "settings"]);

export type ItemDialog =
  | { kind: "rename-folder"; folder: Folder }
  | { kind: "delete-folder"; folder: Folder }
  | { kind: "move-file"; artifact: ArtifactSummary }
  | { kind: "move-folder"; folder: Folder }
  | { kind: "move-files"; artifacts: ArtifactSummary[] }
  | { kind: "delete-files"; artifacts: ArtifactSummary[] }
  | { kind: "delete-file"; artifact: ArtifactSummary };

export type FileCategory = "repository" | "markdown" | "note" | "text" | "code" | "word" | "excel" | "powerpoint" | "pdf" | "onenote" | "email" | "image";

export const FILE_CATEGORIES: Array<{ label: string; value: FileCategory }> = [
  { label: "Markdown", value: "markdown" },
  { label: "Note", value: "note" },
  { label: "Text", value: "text" },
  { label: "Code", value: "code" },
  { label: "Word", value: "word" },
  { label: "Excel", value: "excel" },
  { label: "PowerPoint", value: "powerpoint" },
  { label: "PDF", value: "pdf" },
  { label: "OneNote", value: "onenote" },
  { label: "Email", value: "email" },
  { label: "Image", value: "image" },
  { label: "Repository", value: "repository" },
];

export const TASK_COLUMNS: Array<{ status: CollaborationTaskStatus; label: string }> = [
  { status: "open", label: "Open" },
  { status: "in_progress", label: "In progress" },
  { status: "blocked", label: "Blocked" },
  { status: "done", label: "Done" },
];
