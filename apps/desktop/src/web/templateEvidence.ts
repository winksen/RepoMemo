import { FILE_CATEGORIES, type ArtifactViewMode, type FileCategory } from "./constants";
import type { TemplateEvidence } from "../generated/domain";

/** The view the ledger opens in for a workspace that has not chosen one: its template's, or the grid. */
export function defaultLedgerView(evidence: Pick<TemplateEvidence, "default_view"> | null | undefined): ArtifactViewMode {
  return evidence?.default_view === "list" ? "list" : "grid";
}

/**
 * The file types of the ledger's filter, the template's preferred ones first in its order and the rest after them
 * in the usual order. A type this client does not list is ignored, so a newer template cannot break the menu.
 */
export function orderedFileCategories(evidence: Pick<TemplateEvidence, "types_first"> | null | undefined): typeof FILE_CATEGORIES {
  const first = (evidence?.types_first ?? [])
    .map((value) => FILE_CATEGORIES.find((category) => category.value === (value as FileCategory)))
    .filter((category, index, all): category is (typeof FILE_CATEGORIES)[number] => Boolean(category) && all.indexOf(category) === index);
  return [...first, ...FILE_CATEGORIES.filter((category) => !first.includes(category))];
}
