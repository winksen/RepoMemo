import type { FileCategory } from "../web/constants";
import { documentExtensions, type DocumentKind } from "./documents";
import type { ArtifactKindFilter } from "./sharedApi";

const DOCUMENT_CATEGORIES: ReadonlySet<FileCategory> = new Set<FileCategory>(["word", "excel", "powerpoint", "pdf", "onenote", "email"]);

/** Whether a file type of the evidence section is one of the document formats. */
export function documentCategory(category: FileCategory): category is FileCategory & DocumentKind {
  return DOCUMENT_CATEGORIES.has(category);
}

/** Every document format: the documents view. */
export const DOCUMENT_KIND: ArtifactKindFilter = { extensions: documentExtensions() };

/**
 * A file type of the evidence section as the server's filter for it, so the server finds the files that
 * `fileCategory` would name that type: a document format by its extension first, then the artifact's own
 * type, and plain text is whatever is left.
 */
export function categoryKind(category: FileCategory): ArtifactKindFilter {
  if (documentCategory(category)) return { extensions: documentExtensions(category) };
  const documents = documentExtensions();
  switch (category) {
    case "image": return { artifact_types: ["image"], not_extensions: documents };
    case "code": return { artifact_types: ["code_file"], not_extensions: documents };
    case "note": return { artifact_types: ["note"], not_extensions: documents };
    case "markdown": return { artifact_types: ["markdown_doc"], not_extensions: documents };
    case "repository": return { artifact_types: ["repository"], not_extensions: documents };
    default: return { not_artifact_types: ["repository", "image", "code_file", "note", "markdown_doc"], not_extensions: documents };
  }
}
