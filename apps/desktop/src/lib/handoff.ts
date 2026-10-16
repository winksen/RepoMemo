import { invoke } from "@tauri-apps/api/core";
import type { ArchiveCounts } from "../types";

/** A local workspace written as an archive in the desktop app's exports folder, waiting to be imported on a server (V0.2.50). */
export interface PreparedArchive {
  path: string;
  file_name: string;
  workspace_name: string;
  size_bytes: number;
  counts: ArchiveCounts;
}

const HANDOFF_KEY = "repomemo.desktop.handoff";

/** Writes a local workspace as an archive. */
export function prepareLocalArchive(workspaceId: string): Promise<PreparedArchive> {
  return invoke<PreparedArchive>("export_workspace_archive", { workspaceId });
}

/** Saves a copy of a prepared archive to a place the person chose. */
export function copyPreparedArchive(archive: PreparedArchive, destination: string): Promise<void> {
  return invoke<void>("copy_export", { path: archive.path, destination });
}

/** Deletes a prepared archive. */
export function discardPreparedArchive(archive: PreparedArchive): Promise<void> {
  return invoke<void>("discard_export", { path: archive.path });
}

/** The archive as a file, for a server's import. */
export async function loadPreparedArchive(archive: PreparedArchive): Promise<File> {
  const bytes = await invoke<ArrayBuffer>("read_export", { path: archive.path });
  return new File([bytes], archive.file_name, { type: "application/gzip" });
}

/** Remembers which prepared archive is waiting for a server, across the move from the local workspace to the server. */
export function rememberHandoff(archive: PreparedArchive | null) {
  try {
    if (archive) window.localStorage.setItem(HANDOFF_KEY, JSON.stringify(archive));
    else window.localStorage.removeItem(HANDOFF_KEY);
  } catch { /* the archive is still on disk and can be saved as a copy */ }
}

export function pendingHandoff(): PreparedArchive | null {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(HANDOFF_KEY) ?? "null") as Partial<PreparedArchive> | null;
    if (parsed && typeof parsed.path === "string" && typeof parsed.file_name === "string" && typeof parsed.workspace_name === "string") return parsed as PreparedArchive;
  } catch { /* unreadable: nothing is waiting */ }
  return null;
}
