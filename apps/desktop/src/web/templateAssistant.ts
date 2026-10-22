import type { TemplateAssistant } from "../generated/domain";

/**
 * The assistant's capabilities with the template's preferred ones first, in its order; the others follow in the
 * order the server gave. An id the template names that the server did not offer is ignored.
 */
export function orderCapabilities<T extends { id: string }>(capabilities: T[], assistant: Pick<TemplateAssistant, "order"> | null | undefined): T[] {
  const first = (assistant?.order ?? [])
    .map((id) => capabilities.find((capability) => capability.id === id))
    .filter((capability, index, all): capability is T => capability !== undefined && all.indexOf(capability) === index);
  return [...first, ...capabilities.filter((capability) => !first.includes(capability))];
}

/** The questions the template suggests, as many as it gave (the server holds it to six), without blanks or repeats. */
export function suggestedQuestions(assistant: Pick<TemplateAssistant, "suggestions"> | null | undefined): string[] {
  return (assistant?.suggestions ?? []).map((text) => text.trim()).filter((text, index, all) => text && all.indexOf(text) === index);
}

/** The kinds of evidence the Retrieval page suggests as chips, only those the workspace has files of when that is known. */
export function retrievalChips(chips: string[] | null | undefined, present: string[] | null | undefined): string[] {
  const wanted = (chips ?? []).filter((kind, index, all) => all.indexOf(kind) === index);
  return present ? wanted.filter((kind) => present.includes(kind)) : wanted;
}
