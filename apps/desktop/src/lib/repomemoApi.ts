import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, Workspace } from "../types";

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
