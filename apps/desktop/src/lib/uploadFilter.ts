import { MAX_UPLOAD_BYTES, SKIPPED_UPLOAD_FOLDERS, UPLOAD_EXTENSIONS } from "../web/constants";
import { DOCUMENT_ACCEPT } from "./documents";

/** What an upload keeps from the files someone chose, and why the rest is left out. */
export interface UploadSelection {
  kept: File[];
  unsupported: number;
  tooLarge: number;
  /** In a hidden, dependency or build folder of a chosen folder. */
  ignored: number;
}

/**
 * Keeps the files the upload accepts. Files from a chosen folder keep their
 * folder path (`webkitRelativePath`); hidden, dependency and build folders are
 * skipped as in desktop folder import. `documentsOnly` is the Documents view,
 * which takes business documents and nothing else.
 */
export function selectUploads(files: File[], documentsOnly: boolean): UploadSelection {
  const documentExtensions = DOCUMENT_ACCEPT.split(",").map((entry) => entry.slice(1));
  const accepted = new Set(documentsOnly ? documentExtensions : [...UPLOAD_EXTENSIONS, ...documentExtensions]);
  const selection: UploadSelection = { kept: [], unsupported: 0, tooLarge: 0, ignored: 0 };
  for (const file of files) {
    const folders = (file.webkitRelativePath || "").split("/").slice(0, -1);
    if (folders.some((folder) => SKIPPED_UPLOAD_FOLDERS.has(folder) || folder.startsWith("."))) { selection.ignored += 1; continue; }
    if (!accepted.has(file.name.split(".").pop()?.toLowerCase() ?? "")) { selection.unsupported += 1; continue; }
    if (file.size > MAX_UPLOAD_BYTES) { selection.tooLarge += 1; continue; }
    selection.kept.push(file);
  }
  return selection;
}
