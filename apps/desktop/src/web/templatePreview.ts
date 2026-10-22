import type { TemplateDetail } from "../generated/domain";
import { FILE_CATEGORIES, WORKSPACE_SECTIONS, type WorkspaceSection } from "./constants";
import { artifactTypeLabel } from "./format";
import { isPulseCard, PULSE_CARD_TITLES } from "./workspace/pulseCards";

/** The words for the sections as the tabs use them when a template renames none. */
export const SECTION_NAMES: Record<WorkspaceSection, string> = {
  overview: "Overview", evidence: "Evidence", documents: "Documents", repositories: "Repositories", retrieval: "Retrieval", map: "Map",
  health: "Health", review: "Review", memory: "Memory", tasks: "Tasks", people: "People & Activity", settings: "Settings",
};

type Previewable = Pick<TemplateDetail, "sections" | "labels" | "overview" | "assistant" | "ranking" | "health" | "review" | "map" | "repositories">;

/** What the assistant suggests and recognises, in words. */
function assistantLine(assistant: TemplateDetail["assistant"]): string {
  const plural = (count: number, noun: string) => `${count} ${noun}${count === 1 ? "" : "s"}`;
  const parts = [
    assistant.suggestions.length ? plural(assistant.suggestions.length, "question") : "",
    assistant.phrases.length ? `${plural(assistant.phrases.length, "phrase")} it recognises` : "",
  ].filter(Boolean);
  return parts.length ? parts.join(", ") : "nothing";
}

/** What the link form is filled in with, in words. */
function repositoryRules(repositories: TemplateDetail["repositories"]): string {
  const rules = (count: number, kind: string) => `${count} ${kind} rule${count === 1 ? "" : "s"}`;
  const parts = [
    repositories.include.length ? rules(repositories.include.length, "include") : "",
    repositories.exclude.length ? rules(repositories.exclude.length, "exclude") : "",
  ].filter(Boolean);
  return parts.length ? parts.join(", ") : "nothing";
}

/** What the map's kinds of link are called. */
const LINK_NAMES: Record<string, string> = { similar: "similar files", cites: "memory citations" };

/** What the knowledge map puts first, in words. */
function mapFirst(map: TemplateDetail["map"]): string {
  const types = map.coverage_first.map((type) => FILE_CATEGORIES.find((category) => category.value === type)?.label ?? type);
  const links = map.relations_first.map((link) => LINK_NAMES[link] ?? link);
  const parts = [types.join(", "), links.join(", ")].filter(Boolean);
  return parts.length ? parts.join("; ") : "nothing in particular";
}

/** What the review queue calls its three kinds of item, in the plural. */
const REVIEW_NAMES: Record<string, string> = { evidence: "files", memory_card: "memory cards", health_finding: "open health findings" };

/** What each health check is called on the Health page. */
export const HEALTH_NAMES: Record<string, string> = {
  older_version_active: "Older versions", duplicate_content: "Duplicates", removed_symbol_mentioned: "Removed code",
  outdated_evidence_referenced: "Outdated references", stale_memory_citation: "Stale memory", index_failed: "Indexing failures", unconnected: "Unconnected",
  stale_evidence: "Stale evidence",
};

const isSection = (value: string): value is WorkspaceSection => (WORKSPACE_SECTIONS as string[]).includes(value);

const byShare = (factor: number) => `${Math.round((factor - 1) * 100)} %`;

/** What search favours, in words: verified evidence and each boost by a share, or nothing in particular. */
function favoured(ranking: TemplateDetail["ranking"]): string {
  const parts: string[] = [];
  if (Math.round((ranking.verified - 1) * 100) !== 0) parts.push(`verified evidence by ${byShare(ranking.verified)}`);
  for (const boost of ranking.boosts) {
    const what = boost.artifact_type ? artifactTypeLabel(boost.artifact_type as Parameters<typeof artifactTypeLabel>[0])
      : boost.document_kind ? (FILE_CATEGORIES.find((category) => category.value === boost.document_kind)?.label ?? boost.document_kind)
      : boost.path ?? "";
    if (what) parts.push(`${what} by ${byShare(boost.factor)}`);
  }
  if (ranking.recency) parts.push(`recent evidence by up to ${byShare(ranking.recency.max_factor)}`);
  return parts.length ? parts.join(", ") : "nothing in particular";
}

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
    { label: "Map shows first", text: mapFirst(template.map) },
    { label: "Review lists", text: template.review.order.length ? template.review.order.map((source) => REVIEW_NAMES[source] ?? source).join(", then ") : "everything mixed, the oldest first" },
    { label: "Health checks", text: template.health.detectors.map((detector) => detector === "stale_evidence" && template.health.stale_after_days ? `${HEALTH_NAMES[detector]} (${template.health.stale_after_days} days)` : HEALTH_NAMES[detector] ?? detector).join(", ") },
    { label: "Search favours", text: favoured(template.ranking) },
    { label: "The assistant suggests", text: assistantLine(template.assistant) },
    { label: "Repository links start with", text: repositoryRules(template.repositories) },
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
