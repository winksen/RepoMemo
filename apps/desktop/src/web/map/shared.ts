import type { ReactNode } from "react";
import type { KnowledgeNode } from "../../types";

/** Files are coloured by one of three groups: past three hues, dots that can
 *  sit next to any other dot stop being distinguishable for colour-blind
 *  readers. The palette is validated in both themes (see the CSS tokens). */
export type FileGroup = "documents" | "notes" | "code";

export const GROUP_LABEL: Record<FileGroup, string> = {
  documents: "Documents & images",
  notes: "Notes & text",
  code: "Code & config",
};

export const CANVAS_WIDTH = 900;
export const CANVAS_HEIGHT = 520;
export const POLL_INTERVAL_MS = 5000;
export const POLL_ATTEMPTS = 120;
/** Largest files labelled on the canvas when nothing is selected. */
export const LABELLED_FILES = 5;

export function fileGroup(node: KnowledgeNode): FileGroup {
  if (node.artifact_type === "repository") return "code";
  const extension = node.path?.split(".").pop()?.toLowerCase() ?? "";
  if (node.artifact_type === "code_file" || node.artifact_type === "api_spec" || ["json", "toml", "yaml", "yml"].includes(extension)) return "code";
  if (node.artifact_type === "file" && !["txt", "log"].includes(extension)) return "documents";
  if (node.artifact_type === "image") return "documents";
  return "notes";
}

export function percent(part: number, whole: number) {
  return whole > 0 ? Math.round((part / whole) * 100) : 0;
}

export function shorten(text: string, length = 28) {
  return text.length > length ? `${text.slice(0, length - 1)}…` : text;
}

export type HoverTip = { x: number; y: number; content: ReactNode };
