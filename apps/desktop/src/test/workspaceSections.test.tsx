import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../web/workspace/data", () => ({ useTemplate: vi.fn() }));

import type { SharedWorkspace, WorkspaceTemplateView } from "../types";
import { WORKSPACE_SECTIONS } from "../web/constants";
import { WorkspaceTopbar } from "../web/Organization";
import { arrangeSections } from "../web/templateSections";
import { useTemplate } from "../web/workspace/data";

beforeEach(() => { vi.clearAllMocks(); });
afterEach(cleanup);

const knowledgeBase = {
  primary: ["overview", "documents", "retrieval", "evidence", "review", "health", "memory", "tasks", "people", "settings"],
  more: ["map", "repositories"],
};

describe("arrangeSections", () => {
  it("is the usual twelve in the usual order without a template, and ten for someone who does not administer", () => {
    expect(arrangeSections(null, "overview", true)).toEqual({ tabs: [...WORKSPACE_SECTIONS], more: [] });
    expect(arrangeSections(undefined, "overview", false).tabs).toEqual(WORKSPACE_SECTIONS.filter((section) => section !== "people" && section !== "settings"));
  });

  it("follows the template's order and keeps its lesser sections for the More menu", () => {
    const arranged = arrangeSections(knowledgeBase, "overview", true);
    expect(arranged.tabs).toEqual(knowledgeBase.primary);
    expect(arranged.more).toEqual(["map", "repositories"]);
  });

  it("keeps the administrators' sections for administrators whatever the template says", () => {
    const arranged = arrangeSections(knowledgeBase, "overview", false);
    expect(arranged.tabs).toEqual(knowledgeBase.primary.filter((section) => section !== "people" && section !== "settings"));
    expect(arranged.tabs.concat(arranged.more)).not.toContain("settings");
    // Even a template that puts them under More does not show them to a member.
    expect(arrangeSections({ primary: ["overview"], more: ["people", "settings", "map"] }, "overview", false).more).toEqual(["map"]);
  });

  it("shows the open section as a tab even when the template keeps it under More", () => {
    const arranged = arrangeSections(knowledgeBase, "map", true);
    expect(arranged.tabs[arranged.tabs.length - 1]).toBe("map");
    expect(arranged.more).toEqual(["repositories"]);
    // Opening a section that is already a tab changes nothing.
    expect(arrangeSections(knowledgeBase, "tasks", true).tabs).toEqual(knowledgeBase.primary);
  });

  it("gives a tab to a section the template does not name, ignores one this client has no page for and drops repeats", () => {
    const arranged = arrangeSections({ primary: ["review", "dashboard", "review"], more: ["map"] }, "overview", true);
    expect(arranged.tabs.slice(0, 1)).toEqual(["review"]);
    expect(arranged.tabs).toContain("overview");
    expect(arranged.tabs).not.toContain("dashboard" as never);
    expect(arranged.tabs.filter((section) => section === "review")).toHaveLength(1);
    expect([...arranged.tabs, ...arranged.more].sort()).toEqual([...WORKSPACE_SECTIONS].sort());
    // A section named in both lists (a server never sends that) is kept under More only.
    const both = arrangeSections({ primary: ["map"], more: ["map"] }, "overview", true);
    expect(both.more).toEqual(["map"]);
    expect(both.tabs).not.toContain("map");
  });
});

function workspace(role: SharedWorkspace["role"]): SharedWorkspace {
  return { workspace: { id: "w1", name: "Ops", created_at: "", updated_at: "", settings: {} }, organization_id: "o1", role };
}

function templateView(sections: WorkspaceTemplateView["sections"]): WorkspaceTemplateView {
  return {
    template: { id: "knowledge-base", version: 1, name: "Knowledge base", summary: "", icon: "book" },
    source: "workspace", reason: null, chosen_id: "knowledge-base", overrides: null, sections,
  };
}

describe("the workspace tabs", () => {
  const tabNames = () => screen.getAllByRole("button").map((button) => button.textContent?.replace("Admin", "").trim());

  it("are in the usual order, with no More menu, while the workspace has no template", () => {
    vi.mocked(useTemplate).mockReturnValue([null, vi.fn(), {} as never]);
    render(<WorkspaceTopbar accessToken="t" onNavigate={vi.fn()} workspace={workspace("owner")} />);
    expect(tabNames()).toEqual(["Overview", "Evidence", "Documents", "Repositories", "Retrieval", "Map", "Health", "Review", "Memory", "Tasks", "People & Activity", "Settings"]);
  });

  it("follow the template and end with a More menu for what it keeps out of the row", () => {
    vi.mocked(useTemplate).mockReturnValue([templateView(knowledgeBase), vi.fn(), {} as never]);
    render(<WorkspaceTopbar accessToken="t" activeSection="overview" onNavigate={vi.fn()} workspace={workspace("owner")} />);
    expect(tabNames()).toEqual(["Overview", "Documents", "Retrieval", "Evidence", "Review", "Health", "Memory", "Tasks", "People & Activity", "Settings", "More"]);
  });

  it("show the open section even when it is under More, and ask the server for the template of their own workspace only with a token", () => {
    vi.mocked(useTemplate).mockReturnValue([templateView(knowledgeBase), vi.fn(), {} as never]);
    render(<WorkspaceTopbar accessToken="t" activeSection="map" onNavigate={vi.fn()} workspace={workspace("member")} />);
    expect(tabNames()).toEqual(["Overview", "Documents", "Retrieval", "Evidence", "Review", "Health", "Memory", "Tasks", "Map", "More"]);
    expect(useTemplate).toHaveBeenCalledWith("t", "w1", true);
    cleanup();
    render(<WorkspaceTopbar workspace={workspace("member")} />);
    expect(useTemplate).toHaveBeenLastCalledWith("", "w1", false);
  });
});
