export interface Workspace {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
  settings: Record<string, unknown>;
}

export interface AppSettings {
  data_dir: string;
  ai_enabled: boolean;
  active_provider: string | null;
}

export type SourceType = "upload" | "folder" | "git_repo" | "manual" | "connector";

export type ArtifactType =
  | "file"
  | "markdown_doc"
  | "code_file"
  | "issue"
  | "pr"
  | "decision"
  | "incident"
  | "runbook"
  | "api_spec"
  | "note";

export interface WorkspaceOverview {
  workspace_id: string;
  source_count: number;
  artifact_count: number;
  chunk_count: number;
  symbol_count: number;
  memory_card_count: number;
}

export interface ArtifactSummary {
  id: string;
  workspace_id: string;
  source_id: string;
  source_name: string;
  artifact_type: ArtifactType;
  title: string;
  path: string;
  content_hash: string;
  mime_type: string | null;
  language: string | null;
  size_bytes: number;
  created_at: string;
  updated_at: string;
  indexed_at: string | null;
}

export interface ArtifactDetail {
  summary: ArtifactSummary;
  metadata: Record<string, unknown>;
  content_preview: string | null;
  content_truncated: boolean;
}

export interface ImportSkippedItem {
  path: string;
  reason: string;
}

export interface ImportReport {
  workspace_id: string;
  scanned: number;
  imported: number;
  duplicates: number;
  skipped: number;
  failed: number;
  imported_artifacts: ArtifactSummary[];
  skipped_items: ImportSkippedItem[];
}
