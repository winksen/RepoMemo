import type { ArtifactSummary } from "../types";

export type DocumentKind = "word" | "excel" | "powerpoint" | "pdf" | "onenote" | "email";

const KIND_BY_EXTENSION: Record<string, DocumentKind> = {
  doc: "word",
  docx: "word",
  xlsx: "excel",
  xlsm: "excel",
  xls: "excel",
  pptx: "powerpoint",
  ppt: "powerpoint",
  pdf: "pdf",
  one: "onenote",
  eml: "email",
  msg: "email",
};

/** The extensions of one kind of document, or of all of them. */
export function documentExtensions(kind?: DocumentKind): string[] {
  return Object.entries(KIND_BY_EXTENSION).filter(([, own]) => !kind || own === kind).map(([extension]) => extension);
}

export const DOCUMENT_ACCEPT = Object.keys(KIND_BY_EXTENSION).map((extension) => `.${extension}`).join(",");

export const DOCUMENT_KIND_LABEL: Record<DocumentKind, string> = {
  word: "Word",
  excel: "Excel",
  powerpoint: "PowerPoint",
  pdf: "PDF",
  onenote: "OneNote",
  email: "Email",
};

export function documentKindOf(artifact: Pick<ArtifactSummary, "path">): DocumentKind | null {
  const extension = artifact.path.split(".").pop()?.toLowerCase() ?? "";
  return KIND_BY_EXTENSION[extension] ?? null;
}

/**
 * Formats that can carry macros. They are read (text, tables, slides) and downloaded, but *Open in Office* is
 * not offered: that opens the file straight in the application from a link (V0.2.147).
 */
const MACRO_EXTENSIONS = new Set(["xlsm", "xlsb", "xltm", "xlam", "docm", "dotm", "pptm", "potm", "ppsm", "sldm", "ppam"]);

export function hasMacros(artifact: Pick<ArtifactSummary, "path">): boolean {
  return MACRO_EXTENSIONS.has(artifact.path.split(".").pop()?.toLowerCase() ?? "");
}

/** Whether *Open in …* is offered for this file: its kind has an application, and it cannot carry macros. */
export function canOpenInApp(artifact: Pick<ArtifactSummary, "path">): boolean {
  const kind = documentKindOf(artifact);
  return kind !== null && !hasMacros(artifact);
}

export function isDocumentArtifact(artifact: Pick<ArtifactSummary, "path">): boolean {
  return documentKindOf(artifact) !== null;
}

/**
 * How "Open in ..." reaches the right application. Office apps register URL
 * schemes that fetch a document from a link (read-only view); PDFs open in a
 * browser tab; Outlook messages are saved so the system opens them in the
 * user's mail app.
 */
export const OPEN_IN_APP: Record<DocumentKind, { label: string; scheme: string | null }> = {
  word: { label: "Word", scheme: "ms-word:ofv|u|" },
  excel: { label: "Excel", scheme: "ms-excel:ofv|u|" },
  powerpoint: { label: "PowerPoint", scheme: "ms-powerpoint:ofv|u|" },
  onenote: { label: "OneNote", scheme: "onenote:" },
  pdf: { label: "browser", scheme: null },
  email: { label: "Outlook", scheme: null },
};

export function columnLetters(index: number): string {
  let value = index + 1;
  let letters = "";
  while (value > 0) {
    const remainder = (value - 1) % 26;
    letters = String.fromCharCode(65 + remainder) + letters;
    value = Math.floor((value - 1) / 26);
  }
  return letters;
}
