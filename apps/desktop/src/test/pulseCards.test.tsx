import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { WorkspaceMetrics, WorkspaceOverview } from "../types";
import { DEFAULT_PULSE_CARDS, PULSE_CARD_IDS, pulseCard, pulseCardIds } from "../web/workspace/pulseCards";

afterEach(cleanup);

const metrics = {
  artifact_count: 20, indexed_artifact_count: 15, pending_artifact_count: 5, indexed_artifact_bytes: 2048, pending_artifact_bytes: 1024,
  artifacts_updated_last_7_days: 4, artifacts_created_last_7_days: 3, chunk_count: 300, symbol_count: 42, member_count: 6, source_count: 3,
  open_task_count: 5, in_progress_task_count: 2, blocked_task_count: 1, overdue_task_count: 2, completed_task_count: 9, comment_count: 11,
  repository_count: 4, repositories_in_sync: 3, reviewed_last_90_days: 12, needing_review_files: 2, needing_review_cards: 1, duplicate_file_count: 3,
} as unknown as WorkspaceMetrics;
const overview = { memory_card_count: 8 } as unknown as WorkspaceOverview;

/** The labels and figures of the cards, in the order they are drawn. */
function drawn(ids: Parameters<typeof pulseCard>[0][], withData = true) {
  const { container } = render(<div>{ids.map((id) => pulseCard(id, withData ? metrics : null, withData ? overview : null))}</div>);
  return [...(container.firstElementChild?.children ?? [])].map((card) => [card.querySelector("span")?.textContent, card.querySelector("strong")?.textContent]);
}

describe("the Overview's pulse cards", () => {
  it("are the nine a workspace has always shown, in their order, when nothing else is asked", () => {
    expect(pulseCardIds(undefined)).toEqual(DEFAULT_PULSE_CARDS);
    expect(pulseCardIds(null)).toEqual(DEFAULT_PULSE_CARDS);
    expect(DEFAULT_PULSE_CARDS).toHaveLength(9);
    expect(drawn(DEFAULT_PULSE_CARDS).map(([label]) => label)).toEqual(["Index coverage", "Indexed storage", "Fresh evidence", "Knowledge density", "Code symbols", "Team access", "Active tasks", "At risk", "Team discussion"]);
  });

  it("follow the template's list and its order, leaving out the cards it does not name", () => {
    expect(pulseCardIds(["at_risk", "memory_cards", "code_symbols"])).toEqual(["at_risk", "memory_cards", "code_symbols"]);
    expect(drawn(pulseCardIds(["at_risk", "memory_cards", "code_symbols"]))).toEqual([["At risk", "1"], ["Memory cards", "8"], ["Code symbols", "42"]]);
  });

  it("ignore a card this client cannot draw and a card listed twice, and fall back to the usual nine when none is left", () => {
    expect(pulseCardIds(["at_risk", "revenue", "at_risk"])).toEqual(["at_risk"]);
    expect(pulseCardIds(["revenue"])).toEqual(DEFAULT_PULSE_CARDS);
    expect(pulseCardIds([])).toEqual(DEFAULT_PULSE_CARDS);
  });

  it("draw every card the format names, from figures the page has already loaded", () => {
    const cards = drawn([...PULSE_CARD_IDS]);
    expect(cards).toHaveLength(PULSE_CARD_IDS.length);
    expect(cards.every(([label, figure]) => Boolean(label) && Boolean(figure))).toBe(true);
    const byLabel = Object.fromEntries(cards);
    expect(byLabel).toMatchObject({ "Index coverage": "75%", "Sources": "3", "Memory cards": "8", "Active tasks": "7", "Fresh evidence": "4" });
  });

  it("include the four a template could not name before: repositories in sync, reviews of 90 days, what needs review and duplicates", () => {
    const ids = ["repositories_in_sync", "reviewed_90_days", "needing_review", "duplicates"] as const;
    expect(drawn([...ids])).toEqual([["Repositories in sync", "3"], ["Reviewed in 90 days", "12"], ["Needing review", "3"], ["Duplicates", "3"]]);
    const { container } = render(<div>{ids.map((id) => pulseCard(id, metrics, overview))}</div>);
    const notes = [...container.querySelectorAll("small")].map((note) => note.textContent);
    expect(notes).toEqual(["of 4 linked", "of 20 files", "2 files, 1 memory cards", "files that repeat another's content"]);
    expect(pulseCardIds([...ids])).toEqual([...ids]);
    // None is a default: General's nine are unchanged.
    expect(DEFAULT_PULSE_CARDS.some((id) => (ids as readonly string[]).includes(id))).toBe(false);
    expect(drawn([...ids], false).map(([, figure]) => figure)).toEqual(["—", "—", "—", "—"]);
  });

  it("show a dash for a figure that has not arrived", () => {
    expect(drawn(["index_coverage", "memory_cards", "sources"], false).map(([, figure]) => figure)).toEqual(["—", "—", "—"]);
  });
});
