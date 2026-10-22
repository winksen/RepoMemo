import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ saveSharedWorkspaceTemplate: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));
vi.mock("../web/workspace/data", () => ({ useTemplate: vi.fn(), useTemplateCatalogue: vi.fn(), useTemplateDetail: vi.fn() }));

import { TemplateSettings } from "../components/TemplateSettings";
import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { showToast } from "../components/ui/toast";
import { saveSharedWorkspaceTemplate } from "../lib/sharedApi";
import type { TemplateDetail, TemplateSummary, WorkspaceTemplateView } from "../types";
import { adjustmentsFrom, previewLines, questionsFromText } from "../web/templatePreview";
import { useTemplate, useTemplateCatalogue, useTemplateDetail } from "../web/workspace/data";

const summary = (id: string, name: string): TemplateSummary => ({ id, version: 1, name, summary: `${name} summary`, icon: "book" });
const catalogue = [summary("general", "General"), summary("knowledge-base", "Knowledge base"), summary("operations", "Operations and incidents")];

const operationsDetail: TemplateDetail = {
  template: catalogue[2],
  sections: { primary: ["overview", "evidence", "review", "tasks", "people", "settings"], more: ["map"], landing: "review" },
  labels: { sections: { tasks: "Actions" }, artifact_types: {} },
  overview: { cards: ["fresh_evidence", "at_risk"] },
  evidence: { default_view: "list", types_first: [] },
  assistant: { order: [], suggestions: ["What is the runbook for…"] },
  retrieval: { type_chips: [] },
  review: { order: ["health_finding", "evidence", "memory_card"] },
  map: { coverage_first: ["markdown", "note"], relations_first: ["cites"] },
  health: { detectors: ["stale_evidence", "outdated_evidence_referenced", "stale_memory_citation", "index_failed"], stale_after_days: 90 },
  ranking: { verified: 1.15, recency: { half_life_days: 90, max_factor: 1.2 }, boosts: [{ artifact_type: "runbook", document_kind: null, path: null, factor: 1.2 }, { artifact_type: null, document_kind: "pdf", path: null, factor: 1.15 }, { artifact_type: null, document_kind: null, path: "docs/adr/**", factor: 1.15 }] },
};
const knowledgeDetail: TemplateDetail = {
  ...operationsDetail,
  template: catalogue[1],
  sections: { primary: ["overview", "documents", "people", "settings"], more: [], landing: "documents" },
  labels: { sections: { evidence: "Library" }, artifact_types: {} },
  overview: { cards: ["index_coverage"] },
  assistant: { order: [], suggestions: [] },
};

function current(overrides: WorkspaceTemplateView["overrides"] = null, base: TemplateDetail = operationsDetail): WorkspaceTemplateView {
  return { ...base, source: "workspace", reason: null, chosen_id: base.template.id, overrides };
}

const setCurrent = vi.fn();
function show(view: WorkspaceTemplateView | null) {
  vi.mocked(useTemplate).mockReturnValue([view, setCurrent, {} as never]);
  vi.mocked(useTemplateCatalogue).mockReturnValue([catalogue, vi.fn(), {} as never]);
  vi.mocked(useTemplateDetail).mockImplementation(((_token: string, id: string) => [id === "knowledge-base" ? knowledgeDetail : operationsDetail, vi.fn(), {}]) as never);
  return render(<ConfirmProvider><TemplateSettings accessToken="t" workspaceId="w1" /></ConfirmProvider>);
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(saveSharedWorkspaceTemplate).mockResolvedValue(current());
});
afterEach(cleanup);

describe("the lines that preview a template", () => {
  it("say what it arranges: the tabs, what is under More, where it opens, what it renames, its cards and its questions", () => {
    const lines = Object.fromEntries(previewLines(operationsDetail).map((line) => [line.label, line.text]));
    expect(lines).toEqual({
      "Tabs": "Overview, Evidence, Review, Actions",
      "Under More": "Map",
      "Opens on": "Review",
      "Renamed": "Tasks → Actions",
      "Overview cards": "Fresh evidence, At risk",
      "The assistant suggests": "1 question",
      "Map shows first": "Markdown, Note; memory citations",
      "Review lists": "open health findings, then files, then memory cards",
      "Health checks": "Stale evidence (90 days), Outdated references, Stale memory, Indexing failures",
      "Search favours": "verified evidence by 15 %, Runbook by 20 %, PDF by 15 %, docs/adr/** by 15 %, recent evidence by up to 20 %",
    });
  });

  it("say nothing renamed, nothing under More and nothing suggested when the template does none of it", () => {
    const lines = Object.fromEntries(previewLines({ ...knowledgeDetail, labels: { sections: {}, artifact_types: {} }, review: { order: [] }, map: { coverage_first: [], relations_first: [] } }).map((line) => [line.label, line.text]));
    expect(lines["Renamed"]).toBe("nothing");
    expect(lines["Under More"]).toBe("nothing");
    expect(lines["The assistant suggests"]).toBe("nothing");
    expect(lines["Opens on"]).toBe("Documents");
    expect(lines["Review lists"]).toBe("everything mixed, the oldest first");
    expect(lines["Map shows first"]).toBe("nothing in particular");
    expect(Object.fromEntries(previewLines({ ...knowledgeDetail, ranking: { verified: 1.1, boosts: [], recency: null } }).map((line) => [line.label, line.text]))["Search favours"]).toBe("verified evidence by 10 %");
    expect(Object.fromEntries(previewLines({ ...knowledgeDetail, ranking: { verified: 0.9, boosts: [], recency: null } }).map((line) => [line.label, line.text]))["Search favours"]).toBe("verified evidence by -10 %");
    expect(Object.fromEntries(previewLines({ ...knowledgeDetail, ranking: { verified: 1, boosts: [], recency: null } }).map((line) => [line.label, line.text]))["Search favours"]).toBe("nothing in particular");
  });
});

describe("the adjustments to save", () => {
  const wanted = { landing: "review", more: ["map"], suggestions: ["What is the runbook for…"] };

  it("are nothing when nothing differs from the template, so the template keeps leading", () => {
    expect(adjustmentsFrom(operationsDetail, wanted)).toEqual({});
  });

  it("hold only what differs: the section it opens on, what is under More, the questions", () => {
    expect(adjustmentsFrom(operationsDetail, { ...wanted, landing: "memory" })).toEqual({ landing: "memory" });
    expect(adjustmentsFrom(operationsDetail, { ...wanted, more: ["tasks", "map"] })).toEqual({ more: ["map", "tasks"] });
    expect(adjustmentsFrom(operationsDetail, { ...wanted, more: [] })).toEqual({ more: [] });
    expect(adjustmentsFrom(operationsDetail, { ...wanted, suggestions: [] })).toEqual({ suggestions: [] });
    expect(adjustmentsFrom(operationsDetail, { landing: "memory", more: [], suggestions: ["Who is on call?"] })).toEqual({ landing: "memory", more: [], suggestions: ["Who is on call?"] });
  });

  it("read the questions typed one on each line, trimmed and without blanks", () => {
    expect(questionsFromText("  Who is on call?  \n\n What is the runbook for…\r\n")).toEqual(["Who is on call?", "What is the runbook for…"]);
    expect(questionsFromText("")).toEqual([]);
  });
});

describe("Settings › Template", () => {
  it("shows nothing until the workspace's template has arrived", () => {
    const { container } = show(null);
    expect(container.childElementCount).toBe(0);
  });

  it("shows the template in use with its preview and offers no change while nothing else is chosen", () => {
    const { container } = show(current());
    expect(screen.getByRole("heading", { name: /Template/ })).toBeTruthy();
    expect((screen.getByRole("radio", { name: /Operations and incidents/ }) as HTMLInputElement).checked).toBe(true);
    const preview = container.querySelector("dl");
    expect(preview?.getAttribute("aria-label")).toBe("What the Operations and incidents template arranges");
    expect(preview?.textContent).toContain("Tasks → Actions");
    expect(screen.queryByRole("button", { name: /^Use / })).toBeNull();
  });

  it("previews another template when it is picked and changes only after the person confirms", async () => {
    show(current());
    fireEvent.click(screen.getByRole("radio", { name: /Knowledge base/ }));
    expect(screen.getByText("Library", { exact: false })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Use Knowledge base" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Nothing stored is moved, created or deleted/)).toBeTruthy();
    expect(saveSharedWorkspaceTemplate).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Change template" }));
    await waitFor(() => expect(saveSharedWorkspaceTemplate).toHaveBeenCalledWith("t", "w1", "knowledge-base", undefined));
    await waitFor(() => expect(setCurrent).toHaveBeenCalled());
    expect(showToast).toHaveBeenCalledWith("success", "This workspace now uses the Knowledge base template.");
  });

  it("saves nothing when the change is cancelled", async () => {
    show(current());
    fireEvent.click(screen.getByRole("radio", { name: /Knowledge base/ }));
    fireEvent.click(screen.getByRole("button", { name: "Use Knowledge base" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveSharedWorkspaceTemplate).not.toHaveBeenCalled();
  });

  it("saves only the adjustments that differ from the template, and says so when none does", async () => {
    show(current());
    fireEvent.click(screen.getByRole("button", { name: "Save adjustments" }));
    await waitFor(() => expect(saveSharedWorkspaceTemplate).toHaveBeenCalledWith("t", "w1", "operations", {}));
    expect(showToast).toHaveBeenCalledWith("success", "No adjustment differs from the template.");

    vi.mocked(saveSharedWorkspaceTemplate).mockClear();
    fireEvent.change(screen.getByRole("textbox", { name: /Suggested questions/ }), { target: { value: "Who owns the pager?\nWho is on call?" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Actions" }));
    fireEvent.click(screen.getByRole("button", { name: "Save adjustments" }));
    await waitFor(() => expect(saveSharedWorkspaceTemplate).toHaveBeenCalledWith("t", "w1", "operations", { more: ["map", "tasks"], suggestions: ["Who owns the pager?", "Who is on call?"] }));
    expect(showToast).toHaveBeenCalledWith("success", "The adjustments were saved.");
  });

  it("refuses more than six questions before asking the server", () => {
    show(current());
    fireEvent.change(screen.getByRole("textbox", { name: /Suggested questions/ }), { target: { value: "1\n2\n3\n4\n5\n6\n7" } });
    expect((screen.getByRole("button", { name: "Save adjustments" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("Keep it to 6 questions.")).toBeTruthy();
  });

  it("resets to the template's own values only when something was adjusted", async () => {
    const { unmount } = show(current());
    expect((screen.getByRole("button", { name: "Reset to template" }) as HTMLButtonElement).disabled).toBe(true);
    unmount();
    show(current({ landing: "memory" }));
    const reset = screen.getByRole("button", { name: "Reset to template" }) as HTMLButtonElement;
    expect(reset.disabled).toBe(false);
    fireEvent.click(reset);
    await waitFor(() => expect(saveSharedWorkspaceTemplate).toHaveBeenCalledWith("t", "w1", "operations", {}));
    expect(showToast).toHaveBeenCalledWith("success", "The adjustments were removed; the template's own values are back.");
  });

  it("explains why General is in use when the chosen template cannot be used, and offers the others", () => {
    show({ ...current(), source: "fallback", reason: "The template ghost is not available on this server.", chosen_id: "ghost" });
    expect(screen.getByText(/The template ghost is not available on this server/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Save adjustments" })).toBeNull();
    expect(screen.getAllByRole("radio")).toHaveLength(3);
  });

  it("shows the error as a toast and keeps what was shown when the server refuses", async () => {
    vi.mocked(saveSharedWorkspaceTemplate).mockRejectedValue(new Error("Only administrators may do that."));
    show(current({ landing: "memory" }));
    fireEvent.click(screen.getByRole("button", { name: "Reset to template" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "Only administrators may do that."));
    expect(setCurrent).not.toHaveBeenCalled();
  });
});
