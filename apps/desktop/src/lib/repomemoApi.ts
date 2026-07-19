import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppSettings,
  AskAnswer,
  AskRequest,
  ArtifactDetail,
  ArtifactSummary,
  Chunk,
  IndexingJobStatus,
  ImportReport,
  SearchRequest,
  SearchResult,
  ProviderSettings,
  ProviderTestResult,
  SummaryResult,
  Symbol,
  SymbolSearchResult,
  Workspace,
  WorkspaceOverview,
} from "../types";

const isTauriRuntime = "__TAURI_INTERNALS__" in window;

const mockWorkspaces: Workspace[] = [
  {
    id: "preview-workspace",
    name: "RepoMemo Preview",
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    settings: {},
  },
];

const mockArtifacts: ArtifactSummary[] = [];
const mockChunks = new Map<string, Chunk[]>();
const mockSymbols = new Map<string, Symbol[]>();
const mockProviders = new Map<string, ProviderSettings[]>();

const emptyOverview = (workspaceId: string): WorkspaceOverview => ({
  workspace_id: workspaceId,
  source_count: 0,
  artifact_count: mockArtifacts.length,
  chunk_count: Array.from(mockChunks.values()).reduce(
    (total, chunks) => total + chunks.length,
    0,
  ),
  symbol_count: 0,
  memory_card_count: 0,
});

export async function listWorkspaces(): Promise<Workspace[]> {
  if (!isTauriRuntime) {
    return mockWorkspaces;
  }

  return invoke<Workspace[]>("list_workspaces");
}

export async function createWorkspace(name: string): Promise<Workspace> {
  if (!isTauriRuntime) {
    const workspace: Workspace = {
      id: crypto.randomUUID(),
      name,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      settings: {},
    };
    mockWorkspaces.unshift(workspace);
    return workspace;
  }

  return invoke<Workspace>("create_workspace", { name });
}

export async function getAppSettings(): Promise<AppSettings> {
  if (!isTauriRuntime) {
    return {
      data_dir: ".repomemo/preview",
      ai_enabled: false,
      active_provider: null,
    };
  }

  return invoke<AppSettings>("get_app_settings");
}

export async function listProviderSettings(workspaceId: string): Promise<ProviderSettings[]> {
  if (!isTauriRuntime) return mockProviders.get(workspaceId) ?? [];
  return invoke<ProviderSettings[]>("list_provider_settings", { workspaceId });
}

export async function saveProviderSettings(settings: ProviderSettings): Promise<ProviderSettings> {
  if (!isTauriRuntime) {
    const saved = { ...settings, id: settings.id || crypto.randomUUID() };
    const existing = mockProviders.get(saved.workspace_id ?? "") ?? [];
    mockProviders.set(saved.workspace_id ?? "", [...existing.filter((item) => item.id !== saved.id), saved]);
    return saved;
  }
  return invoke<ProviderSettings>("save_provider_settings", { settings });
}

export async function testProvider(providerId: string): Promise<ProviderTestResult> {
  if (!isTauriRuntime) return { provider_id: providerId, success: true, message: "Preview provider is ready." };
  return invoke<ProviderTestResult>("test_provider", { providerId });
}

export async function summarizeArtifact(artifactId: string, providerId: string): Promise<SummaryResult> {
  if (!isTauriRuntime) return { summary_markdown: "Preview summary. Configure a local provider in the desktop app to generate a real one.", citations: [], warnings: [] };
  return invoke<SummaryResult>("summarize_artifact", { artifactId, providerId });
}

export async function summarizeWorkspace(workspaceId: string, providerId: string): Promise<SummaryResult> {
  if (!isTauriRuntime) return { summary_markdown: "Preview workspace summary. Configure a provider in Settings to generate a real one.", citations: [], warnings: [] };
  return invoke<SummaryResult>("summarize_workspace", { workspaceId, providerId });
}

export async function embedWorkspace(workspaceId: string, providerId: string): Promise<IndexingJobStatus> {
  if (!isTauriRuntime) return mockIndexingJob(workspaceId, 0, 0, "completed");
  return invoke<IndexingJobStatus>("embed_workspace", { workspaceId, providerId });
}

export async function askWorkspace(request: AskRequest): Promise<AskAnswer> {
  if (!isTauriRuntime) return { answer_markdown: "Indexed context is insufficient for a reliable answer.", citations: [], retrieved_context: [], confidence: 0, warnings: ["Preview mode does not call an AI provider."] };
  return invoke<AskAnswer>("ask_workspace", { request });
}

export const ACCEPTED_TEXT_EXTENSIONS = [
  "md",
  "mdx",
  "txt",
  "rs",
  "ts",
  "tsx",
  "js",
  "jsx",
  "py",
  "json",
  "toml",
  "yaml",
  "yml",
  "sql",
  "html",
  "css",
  "sh",
  "ps1",
];
export const ACCEPTED_IMAGE_EXTENSIONS = [
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "svg",
  "bmp",
];
export const ACCEPTED_EXTENSIONS = [
  ...ACCEPTED_TEXT_EXTENSIONS,
  ...ACCEPTED_IMAGE_EXTENSIONS,
];

export async function chooseImportFiles(): Promise<string[]> {
  if (!isTauriRuntime) {
    return ["preview://architecture.md"];
  }

  const selected = await open({
    multiple: true,
    directory: false,
    title: "Import files into RepoMemo",
    filters: [
      { name: "Text & code", extensions: ACCEPTED_TEXT_EXTENSIONS },
      { name: "Images", extensions: ACCEPTED_IMAGE_EXTENSIONS },
      { name: "All supported", extensions: ACCEPTED_EXTENSIONS },
    ],
  });

  return normalizeDialogSelection(selected);
}

export async function chooseImportFolder(): Promise<string[]> {
  if (!isTauriRuntime) {
    return ["preview://sample-workspace"];
  }

  const selected = await open({
    multiple: false,
    directory: true,
    title: "Import folder into RepoMemo",
  });

  return normalizeDialogSelection(selected);
}

export async function importPaths(
  workspaceId: string,
  paths: string[],
): Promise<ImportReport> {
  if (!isTauriRuntime) {
    const artifact: ArtifactSummary = {
      id: crypto.randomUUID(),
      workspace_id: workspaceId,
      source_id: "preview-source",
      source_name: "Preview import",
      artifact_type: "markdown_doc",
      title: "architecture.md",
      path: "architecture.md",
      content_hash: crypto.randomUUID().replace(/-/g, ""),
      mime_type: "text/markdown",
      language: "Markdown",
      size_bytes: 14822,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      indexed_at: null,
    };
    mockArtifacts.unshift(artifact);
    return {
      workspace_id: workspaceId,
      scanned: paths.length,
      imported: 1,
      duplicates: 0,
      skipped: 0,
      failed: 0,
      imported_artifacts: [artifact],
      skipped_items: [],
    };
  }

  return invoke<ImportReport>("import_paths", {
    workspaceId,
    paths,
  });
}

export const PASTE_LANGUAGES = [
  "Text",
  "Markdown",
  "Rust",
  "TypeScript",
  "JavaScript",
  "Python",
  "JSON",
  "TOML",
  "YAML",
  "SQL",
  "HTML",
  "CSS",
  "Shell",
  "PowerShell",
] as const;
export type PasteLanguage = (typeof PASTE_LANGUAGES)[number];

export async function importText(
  workspaceId: string,
  title: string,
  content: string,
  language: PasteLanguage,
): Promise<ArtifactSummary> {
  if (!isTauriRuntime) {
    const artifact: ArtifactSummary = {
      id: crypto.randomUUID(),
      workspace_id: workspaceId,
      source_id: "preview-paste",
      source_name: "Pasted notes",
      artifact_type: language === "Markdown" ? "markdown_doc" : "file",
      title: title || "Pasted note",
      path: (title || "pasted-note").toLowerCase() + ".txt",
      content_hash: crypto.randomUUID().replace(/-/g, ""),
      mime_type: language === "Markdown" ? "text/markdown" : "text/plain",
      language,
      size_bytes: content.length,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      indexed_at: null,
    };
    mockArtifacts.unshift(artifact);
    return artifact;
  }

  return invoke<ArtifactSummary>("import_text", {
    request: { workspaceId, title, content, language },
  });
}

export async function listArtifacts(
  workspaceId: string,
): Promise<ArtifactSummary[]> {
  if (!isTauriRuntime) {
    return mockArtifacts.filter((artifact) => artifact.workspace_id === workspaceId);
  }

  return invoke<ArtifactSummary[]>("list_artifacts", { workspaceId });
}

export async function getArtifact(
  artifactId: string,
): Promise<ArtifactDetail> {
  if (!isTauriRuntime) {
    const artifact = mockArtifacts.find((item) => item.id === artifactId);
    if (!artifact) {
      throw new Error("Artifact was not found.");
    }
    return {
      summary: artifact,
      metadata: {},
      content_preview:
        "# Preview artifact\n\nThis browser preview does not read local files. Run the Tauri app to import real content.",
      content_truncated: false,
      chunks: mockChunks.get(artifactId) ?? [],
    };
  }

  return invoke<ArtifactDetail>("get_artifact", { artifactId });
}

export async function indexArtifact(
  artifactId: string,
): Promise<IndexingJobStatus> {
  if (!isTauriRuntime) {
    const artifact = mockArtifacts.find((item) => item.id === artifactId);
    if (!artifact) {
      throw new Error("Artifact was not found.");
    }

    artifact.indexed_at = new Date().toISOString();
    mockChunks.set(artifactId, [
      {
        id: crypto.randomUUID(),
        artifact_id: artifactId,
        workspace_id: artifact.workspace_id,
        chunk_index: 0,
        text: "# Preview artifact\n\nThis browser preview renders mock chunks.",
        token_count: 8,
        start_line: 1,
        end_line: 3,
        heading_path: "Preview artifact",
        content_hash: crypto.randomUUID().replace(/-/g, ""),
        embedding_status: "not_configured",
        metadata: {},
      },
    ]);
    if (artifact.language === "TypeScript" || artifact.language === "JavaScript") {
      mockSymbols.set(artifactId, [
        {
          id: crypto.randomUUID(),
          artifact_id: artifactId,
          workspace_id: artifact.workspace_id,
          kind: "function",
          name: "previewSymbol",
          signature: "function previewSymbol()",
          start_line: 1,
          end_line: 3,
          metadata: {},
        },
      ]);
    }

    return mockIndexingJob(artifact.workspace_id, 1, 1, "completed");
  }

  return invoke<IndexingJobStatus>("index_artifact", { artifactId });
}

export async function indexWorkspace(
  workspaceId: string,
): Promise<IndexingJobStatus> {
  if (!isTauriRuntime) {
    const artifacts = mockArtifacts.filter(
      (artifact) => artifact.workspace_id === workspaceId,
    );
    for (const artifact of artifacts) {
      await indexArtifact(artifact.id);
    }

    return mockIndexingJob(
      workspaceId,
      artifacts.length,
      artifacts.length,
      "completed",
    );
  }

  return invoke<IndexingJobStatus>("index_workspace", { workspaceId });
}

export async function getWorkspaceOverview(
  workspaceId: string,
): Promise<WorkspaceOverview> {
  if (!isTauriRuntime) {
    return emptyOverview(workspaceId);
  }

  return invoke<WorkspaceOverview>("get_workspace_overview", { workspaceId });
}

export async function searchWorkspace(
  request: SearchRequest,
): Promise<SearchResult[]> {
  if (!isTauriRuntime) {
    const terms = request.query
      .toLowerCase()
      .split(/\s+/)
      .filter(Boolean);

    return mockArtifacts
      .flatMap((artifact) =>
        (mockChunks.get(artifact.id) ?? []).map((chunk) => ({ artifact, chunk })),
      )
      .filter(({ artifact, chunk }) => {
        if (artifact.workspace_id !== request.workspace_id) return false;
        if (
          request.artifact_types.length > 0 &&
          !request.artifact_types.includes(artifact.artifact_type)
        ) return false;
        if (
          request.languages.length > 0 &&
          (!artifact.language || !request.languages.includes(artifact.language))
        ) return false;
        if (
          request.source_ids.length > 0 &&
          !request.source_ids.includes(artifact.source_id)
        ) return false;
        const searchable = `${artifact.title} ${artifact.path} ${chunk.text}`.toLowerCase();
        return terms.every((term) => searchable.includes(term));
      })
      .slice(0, request.limit ?? 40)
      .map(({ artifact, chunk }) => ({
        artifact_id: artifact.id,
        chunk_id: chunk.id,
        title: artifact.title,
        path: artifact.path,
        artifact_type: artifact.artifact_type,
        language: artifact.language,
        snippet: chunk.text,
        start_line: chunk.start_line,
        end_line: chunk.end_line,
        score: 1,
        source_name: artifact.source_name,
      }));
  }

  return invoke<SearchResult[]>("search_workspace", { request });
}

export async function listSymbols(artifactId: string): Promise<Symbol[]> {
  if (!isTauriRuntime) {
    return mockSymbols.get(artifactId) ?? [];
  }
  return invoke<Symbol[]>("list_symbols", { artifactId });
}

export async function searchSymbols(
  workspaceId: string,
  query: string,
): Promise<SymbolSearchResult[]> {
  if (!isTauriRuntime) {
    const term = query.trim().toLowerCase();
    return mockArtifacts
      .flatMap((artifact) =>
        (mockSymbols.get(artifact.id) ?? []).map((symbol) => ({ artifact, symbol })),
      )
      .filter(
        ({ artifact, symbol }) =>
          artifact.workspace_id === workspaceId &&
          symbol.name.toLowerCase().includes(term),
      )
      .map(({ artifact, symbol }) => ({
        symbol,
        title: artifact.title,
        path: artifact.path,
        language: artifact.language,
        source_name: artifact.source_name,
      }));
  }
  return invoke<SymbolSearchResult[]>("search_symbols", { workspaceId, query });
}

function normalizeDialogSelection(selection: string | string[] | null): string[] {
  if (!selection) {
    return [];
  }

  return Array.isArray(selection) ? selection : [selection];
}

function mockIndexingJob(
  workspaceId: string,
  progressCurrent: number,
  progressTotal: number,
  status: string,
): IndexingJobStatus {
  const now = new Date().toISOString();

  return {
    id: crypto.randomUUID(),
    workspace_id: workspaceId,
    source_id: null,
    status,
    stage: status === "completed" ? "chunked" : "chunking",
    progress_current: progressCurrent,
    progress_total: progressTotal,
    error_message: null,
    created_at: now,
    updated_at: now,
  };
}
