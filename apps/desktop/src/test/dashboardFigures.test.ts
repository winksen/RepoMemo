import { describe, expect, it } from "vitest";
import { cardFigures, headlineText, templateName } from "../web/dashboardFigures";
import type { DashboardTemplate, DashboardWorkspaceFigures } from "../lib/sharedApi";

const figures = (template: DashboardTemplate | null, overrides: Partial<DashboardWorkspaceFigures> = {}): DashboardWorkspaceFigures => ({
  workspace_id: "w1", artifact_count: 12, active_task_count: 3, memory_card_count: 2, template, ...overrides,
});
const software: DashboardTemplate = { id: "software-engineering", name: "Software engineering", headline: { card: "code_symbols", value: 1204 } };
const operations: DashboardTemplate = { id: "operations", name: "Operations and incidents", headline: { card: "active_tasks", value: 3 } };

describe("the template on a workspace's card", () => {
  it("names the template, and says nothing for General", () => {
    expect(templateName(figures(software))).toBe("Software engineering");
    expect(templateName(figures(null))).toBeNull();
    expect(templateName(undefined)).toBeNull();
  });

  it("puts the headline in words, with the pulse card's own name", () => {
    expect(headlineText(software)).toBe("1,204 code symbols");
    expect(headlineText({ ...software, headline: { card: "memory_cards", value: 0 } })).toBe("0 memory cards");
    expect(headlineText({ ...software, headline: null })).toBeNull();
    expect(headlineText(null)).toBeNull();
    // A card this client cannot draw is not shown rather than shown wrongly.
    expect(headlineText({ ...software, headline: { card: "telepathy", value: 4 } })).toBeNull();
  });

  it("adds the headline to the usual figures, and stands out the one that is already there", () => {
    expect(cardFigures(figures(null))).toEqual([
      { key: "artifacts", text: "12 artifacts", headline: false },
      { key: "active_tasks", text: "3 active tasks", headline: false },
    ]);
    expect(cardFigures(figures(software))).toEqual([
      { key: "artifacts", text: "12 artifacts", headline: false },
      { key: "active_tasks", text: "3 active tasks", headline: false },
      { key: "code_symbols", text: "1,204 code symbols", headline: true },
    ]);
    // Active tasks is on every card already: the headline is that figure, not a second one.
    expect(cardFigures(figures(operations)).map((row) => [row.key, row.headline])).toEqual([["artifacts", false], ["active_tasks", true]]);
    expect(cardFigures(figures({ ...software, headline: null })).every((row) => !row.headline)).toBe(true);
  });
});
