import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppSettings,
  ArtifactDetail,
  ArtifactSummary,
  ImportReport,
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

const emptyOverview = (workspaceId: string): WorkspaceOverview => ({
  workspace_id: workspaceId,
  source_count: 0,
  artifact_count: mockArtifacts.length,
  chunk_count: 0,
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

export async function chooseImportFiles(): Promise<string[]> {
  if (!isTauriRuntime) {
    return ["preview://architecture.md"];
  }

  const selected = await open({
    multiple: true,
    directory: false,
    title: "Import files into RepoMemo",
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
    };
  }

  return invoke<ArtifactDetail>("get_artifact", { artifactId });
}

export async function getWorkspaceOverview(
  workspaceId: string,
): Promise<WorkspaceOverview> {
  if (!isTauriRuntime) {
    return emptyOverview(workspaceId);
  }

  return invoke<WorkspaceOverview>("get_workspace_overview", { workspaceId });
}

function normalizeDialogSelection(selection: string | string[] | null): string[] {
  if (!selection) {
    return [];
  }

  return Array.isArray(selection) ? selection : [selection];
}
