import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../web/workspace/data", () => ({ useTemplate: vi.fn() }));
vi.mock("../web/router", async (original) => ({ ...(await original<typeof import("../web/router")>()), navigate: vi.fn() }));

import type { SharedWorkspace, WorkspaceTemplateView } from "../types";
import { WORKSPACE_SECTIONS } from "../web/constants";
import { WorkspaceTopbar } from "../web/Organization";
import { artifactKindLabel, artifactTypeLabel } from "../web/format";
import { pageTitle } from "../web/pageTitle";
import { paletteEntries } from "../web/paletteEntries";
import { navigate, parseRoute, workspaceHomePath } from "../web/router";
import { arrangeSections, landingSection } from "../web/templateSections";
import { WorkspaceLanding } from "../web/WorkspaceLanding";
import { useTemplate } from "../web/workspace/data";

beforeEach(() => { vi.clearAllMocks(); });
afterEach(cleanup);

const knowledgeBase = {
  primary: ["overview", "documents", "retrieval", "evidence", "review", "health", "memory", "tasks", "people", "settings"],
  more: ["map", "repositories"],
  landing: "documents",
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
    expect(arrangeSections({ primary: ["overview"], more: ["people", "settings", "map"], landing: "overview" }, "overview", false).more).toEqual(["map"]);
  });

  it("shows the open section as a tab even when the template keeps it under More", () => {
    const arranged = arrangeSections(knowledgeBase, "map", true);
    expect(arranged.tabs[arranged.tabs.length - 1]).toBe("map");
    expect(arranged.more).toEqual(["repositories"]);
    // Opening a section that is already a tab changes nothing.
    expect(arrangeSections(knowledgeBase, "tasks", true).tabs).toEqual(knowledgeBase.primary);
  });

  it("gives a tab to a section the template does not name, ignores one this client has no page for and drops repeats", () => {
    const arranged = arrangeSections({ primary: ["review", "dashboard", "review"], more: ["map"], landing: "review" }, "overview", true);
    expect(arranged.tabs.slice(0, 1)).toEqual(["review"]);
    expect(arranged.tabs).toContain("overview");
    expect(arranged.tabs).not.toContain("dashboard" as never);
    expect(arranged.tabs.filter((section) => section === "review")).toHaveLength(1);
    expect([...arranged.tabs, ...arranged.more].sort()).toEqual([...WORKSPACE_SECTIONS].sort());
    // A section named in both lists (a server never sends that) is kept under More only.
    const both = arrangeSections({ primary: ["map"], more: ["map"], landing: "overview" }, "overview", true);
    expect(both.more).toEqual(["map"]);
    expect(both.tabs).not.toContain("map");
  });
});

describe("landingSection", () => {
  it("is the template's landing section, or Overview when there is none, it is unknown, or it is for administrators", () => {
    expect(landingSection(knowledgeBase)).toBe("documents");
    expect(landingSection(null)).toBe("overview");
    expect(landingSection(undefined)).toBe("overview");
    expect(landingSection({ landing: "dashboard" })).toBe("overview");
    expect(landingSection({ landing: "settings" })).toBe("overview");
    expect(landingSection({ landing: "people" })).toBe("overview");
  });
});

describe("opening a workspace by its address alone", () => {
  const query = (isError: boolean) => ({ isError }) as never;

  it("names no section in the address that opens a workspace, and reads one without a section as the landing request", () => {
    expect(workspaceHomePath("w 1")).toBe("/workspaces/w%201");
    expect(parseRoute("/workspaces/w1")).toMatchObject({ name: "workspace", workspaceId: "w1", landing: true });
    expect(parseRoute("/workspaces/w1/review")).toEqual({ name: "workspace", workspaceId: "w1", section: "review" });
  });

  it("waits for the template, then replaces its own address with the landing section", () => {
    vi.mocked(useTemplate).mockReturnValue([null, vi.fn(), query(false)]);
    const { rerender } = render(<WorkspaceLanding accessToken="t" workspaceId="w1" />);
    expect(screen.getByRole("status").textContent).toContain("Opening the workspace");
    expect(navigate).not.toHaveBeenCalled();
    vi.mocked(useTemplate).mockReturnValue([templateView(knowledgeBase), vi.fn(), query(false)]);
    rerender(<WorkspaceLanding accessToken="t" workspaceId="w1" />);
    expect(navigate).toHaveBeenCalledWith("/workspaces/w1/documents", true);
  });

  it("opens on Overview when the template cannot be read, as it always did", () => {
    vi.mocked(useTemplate).mockReturnValue([null, vi.fn(), query(true)]);
    render(<WorkspaceLanding accessToken="t" workspaceId="w1" />);
    expect(navigate).toHaveBeenCalledWith("/workspaces/w1/overview", true);
  });
});

describe("the command palette's sections", () => {
  const input = (role: SharedWorkspace["role"], sections?: WorkspaceTemplateView["sections"]) => ({
    query: "", workspaces: [workspace(role)], assistantWorkspace: undefined, currentWorkspace: workspace(role),
    files: [], cards: [], canAdministerSystem: false, sections,
  });
  const labels = (entries: ReturnType<typeof paletteEntries>) => entries.filter((entry) => entry.group === "This workspace").map((entry) => entry.label);

  it("lists them in the usual order without a template", () => {
    expect(labels(paletteEntries(input("owner")))).toEqual(["Overview", "Evidence", "Documents", "Repositories", "Retrieval", "Map", "Health", "Review queue", "Memory", "Tasks", "People", "Workspace settings"]);
  });

  it("lists the tabs in the template's order and the sections kept under More after them", () => {
    expect(labels(paletteEntries(input("owner", knowledgeBase)))).toEqual(["Overview", "Documents", "Retrieval", "Evidence", "Review queue", "Health", "Memory", "Tasks", "People", "Workspace settings", "Map", "Repositories"]);
    expect(labels(paletteEntries(input("member", knowledgeBase)))).toEqual(["Overview", "Documents", "Retrieval", "Evidence", "Review queue", "Health", "Memory", "Tasks", "Map", "Repositories"]);
  });

  it("opens a workspace from the palette on its landing section", () => {
    const entry = paletteEntries(input("owner")).find((candidate) => candidate.group === "Workspaces");
    expect(entry?.action).toEqual({ kind: "navigate", to: "/workspaces/w1" });
  });
});

function workspace(role: SharedWorkspace["role"]): SharedWorkspace {
  return { workspace: { id: "w1", name: "Ops", created_at: "", updated_at: "", settings: {} }, organization_id: "o1", role };
}

function templateView(sections: WorkspaceTemplateView["sections"], labels: WorkspaceTemplateView["labels"] = { sections: {}, artifact_types: {} }): WorkspaceTemplateView {
  return {
    template: { id: "knowledge-base", version: 1, name: "Knowledge base", summary: "", icon: "book", custom: false },
    source: "workspace", reason: null, chosen_id: "knowledge-base", overrides: null, sections, labels, overview: { cards: ["index_coverage"], headline: null }, evidence: { default_view: "grid", types_first: [] }, assistant: { order: [], suggestions: [], phrases: [], focus: null }, retrieval: { type_chips: [] }, ranking: { verified: 1.1, boosts: [], recency: null }, health: { detectors: ["index_failed"], stale_after_days: null }, review: { order: [] }, map: { coverage_first: [], relations_first: [] }, repositories: { include: [], exclude: [] },
  };
}

const operationsNames = { sections: { tasks: "Actions", memory: "Lessons" }, artifact_types: { pr: "Pull request", api_spec: "API spec" } };

describe("the names a template gives", () => {
  it("rename a kind of evidence wherever it is named, and leave the others as they were", () => {
    expect(artifactTypeLabel("pr")).toBe("Pr");
    expect(artifactTypeLabel("pr", operationsNames.artifact_types)).toBe("Pull request");
    expect(artifactTypeLabel("api_spec", operationsNames.artifact_types)).toBe("API spec");
    expect(artifactTypeLabel("runbook", operationsNames.artifact_types)).toBe("Runbook");
    expect(artifactTypeLabel("pr", null)).toBe("Pr");
    // The tag of a file is its language when it has one; otherwise the (renamed) kind, and a note and a repository keep their own tags unless renamed.
    expect(artifactKindLabel({ artifact_type: "pr", language: null }, operationsNames.artifact_types)).toBe("Pull request");
    expect(artifactKindLabel({ artifact_type: "pr", language: "Markdown" }, operationsNames.artifact_types)).toBe("Markdown");
    expect(artifactKindLabel({ artifact_type: "note", language: null })).toBe("Note");
    expect(artifactKindLabel({ artifact_type: "note", language: null }, { note: "Field note" })).toBe("Field note");
    expect(artifactKindLabel({ artifact_type: "repository", language: null })).toBe("Git repository");
  });

  it("are in the title of the page and what a screen reader hears after a move", () => {
    const route = { name: "workspace", workspaceId: "w1", section: "tasks" } as const;
    expect(pageTitle(route, { workspace: "Ops" })).toBe("Ops · Tasks");
    expect(pageTitle(route, { workspace: "Ops", sectionLabels: operationsNames.sections })).toBe("Ops · Actions");
    expect(pageTitle({ ...route, section: "people" }, { workspace: "Ops", sectionLabels: operationsNames.sections })).toBe("Ops · People & Activity");
  });

  it("are how the command palette lists a section and what it matches", () => {
    const base = { workspaces: [workspace("owner")], assistantWorkspace: undefined, currentWorkspace: workspace("owner"), files: [], cards: [], canAdministerSystem: false };
    const named = paletteEntries({ ...base, query: "", labels: operationsNames.sections }).filter((entry) => entry.group === "This workspace").map((entry) => entry.label);
    expect(named).toContain("Actions");
    expect(named).toContain("Lessons");
    expect(named).not.toContain("Tasks");
    expect(paletteEntries({ ...base, query: "actions", labels: operationsNames.sections }).some((entry) => entry.id === "section:tasks")).toBe(true);
    expect(paletteEntries({ ...base, query: "tasks", labels: operationsNames.sections }).some((entry) => entry.id === "section:tasks")).toBe(false);
  });

  it("show on the tabs, with the usual name kept as the tooltip", () => {
    vi.mocked(useTemplate).mockReturnValue([templateView(knowledgeBase, operationsNames), vi.fn(), {} as never]);
    render(<WorkspaceTopbar accessToken="t" activeSection="overview" onNavigate={vi.fn()} workspace={workspace("owner")} />);
    const renamed = screen.getByRole("button", { name: /Actions/ });
    expect(renamed.getAttribute("title")).toBe("Tasks");
    expect(screen.getByRole("button", { name: /Lessons/ }).getAttribute("title")).toBe("Memory");
    expect(screen.queryByRole("button", { name: /^Tasks$/ })).toBeNull();
    // A tab that keeps its name has no tooltip.
    expect(screen.getByRole("button", { name: /Retrieval/ }).getAttribute("title")).toBeNull();
  });
});

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
