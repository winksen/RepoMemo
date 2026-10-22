import type { TemplateDetail } from "../generated/domain";
import { WORKSPACE_SECTIONS, type WorkspaceSection } from "./constants";
import { isPulseCard, PULSE_CARD_TITLES } from "./workspace/pulseCards";

/** The words for the sections as the tabs use them when a template renames none. */
export const SECTION_NAMES: Record<WorkspaceSection, string> = {
  overview: "Overview", evidence: "Evidence", documents: "Documents", repositories: "Repositories", retrieval: "Retrieval", map: "Map",
  health: "Health", review: "Review", memory: "Memory", tasks: "Tasks", people: "People & Activity", settings: "Settings",
};

type Previewable = Pick<TemplateDetail, "sections" | "labels" | "overview" | "assistant">;

const isSection = (value: string): value is WorkspaceSection => (WORKSPACE_SECTIONS as string[]).includes(value);

/** What a template would make of a workspace, in lines a person can read before choosing it: one line for each thing it arranges. */
export function previewLines(template: Previewable): Array<{ label: string; text: string }> {
  const name = (section: string) => template.labels.sections[section] ?? (isSection(section) ? SECTION_NAMES[section] : section);
  const visible = (sections: string[]) => sections.filter((section) => section !== "people" && section !== "settings").map(name);
  const renamed = Object.entries(template.labels.sections).map(([section, label]) => `${isSection(section) ? SECTION_NAMES[section] : section} → ${label}`);
  const cards = template.overview.cards.filter(isPulseCard).map((card) => PULSE_CARD_TITLES[card]);
  return [
    { label: "Tabs", text: visible(template.sections.primary).join(", ") },
    { label: "Under More", text: template.sections.more.length ? visible(template.sections.more).join(", ") : "nothing" },
    { label: "Opens on", text: name(template.sections.landing) },
    { label: "Renamed", text: renamed.length ? renamed.join(", ") : "nothing" },
    { label: "Overview cards", text: cards.join(", ") },
    { label: "The assistant suggests", text: template.assistant.suggestions.length ? `${template.assistant.suggestions.length} question${template.assistant.suggestions.length === 1 ? "" : "s"}` : "nothing" },
  ];
}

const sameList = (a: string[], b: string[]) => a.length === b.length && a.every((entry, index) => entry === b[index]);

/**
 * The adjustments to save: only what differs from the template's own values, so the template's later improvements
 * still reach the workspace for everything that was not adjusted. An empty object means "no adjustments".
 */
export function adjustmentsFrom(base: Pick<TemplateDetail, "sections" | "assistant">, wanted: { landing: string; more: string[]; suggestions: string[] }): Record<string, unknown> {
  const overrides: Record<string, unknown> = {};
  const inOrder = (sections: string[]) => WORKSPACE_SECTIONS.filter((section) => sections.includes(section));
  if (wanted.landing !== base.sections.landing) overrides.landing = wanted.landing;
  if (!sameList(inOrder(wanted.more), inOrder(base.sections.more))) overrides.more = inOrder(wanted.more);
  if (!sameList(wanted.suggestions, base.assistant.suggestions)) overrides.suggestions = wanted.suggestions;
  return overrides;
}

/** The questions typed one on each line: trimmed, without blanks. */
export function questionsFromText(text: string): string[] {
  return text.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
}
