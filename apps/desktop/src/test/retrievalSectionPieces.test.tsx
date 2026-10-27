import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const saved = vi.hoisted(() => ({ list: [] as unknown[] }));
vi.mock("../lib/sharedApi", () => ({
  askSharedWorkspace: vi.fn(),
  createSharedSavedSearch: vi.fn(),
  deleteSharedSavedSearch: vi.fn(),
  searchSharedSymbols: vi.fn(),
  searchSharedWorkspace: vi.fn(),
  setSharedSavedSearchSharing: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));
vi.mock("../web/workspace/data", () => ({
  useFolders: () => [[]],
  useTemplate: () => [null],
  useRetrievalFacets: () => [{ artifact_types: [], languages: ["Rust"], sources: [] }],
  useSavedSearches: () => [saved.list, vi.fn()],
}));
vi.mock("../web/common", () => ({ EvidenceUsed: () => null, renderHighlightedSnippet: (text: string) => text }));

import { RetrievalSection } from "../web/workspace/RetrievalSection";
import { searchSharedWorkspace, setSharedSavedSearchSharing } from "../lib/sharedApi";
import { makePage } from "./workspacePage";

afterEach(() => { cleanup(); saved.list = []; window.history.replaceState(null, "", "/"); vi.clearAllMocks(); });

describe("retrieval on the shared pieces", () => {
  it("labels every filter, with the dropdowns tied to their labels and the dates as fields", () => {
    render(<RetrievalSection page={makePage({ section: "retrieval" })} />);
    for (const label of ["File type", "Language", "Source", "Evidence state", "Folder", "Results"]) {
      expect(screen.getByLabelText(label).closest(".rm-field")).not.toBeNull();
    }
    expect((screen.getByLabelText("Updated from") as HTMLInputElement).type).toBe("date");
    expect((screen.getByLabelText("Updated to") as HTMLInputElement).type).toBe("date");
  });

  it("uses the shared search field for the query and the symbol name, and clears the query with its button", () => {
    render(<RetrievalSection page={makePage({ section: "retrieval" })} />);
    const query = screen.getByLabelText("Search indexed evidence") as HTMLInputElement;
    expect(query.closest(".rm-search-field")).not.toBeNull();
    fireEvent.change(query, { target: { value: "login" } });
    expect((screen.getByLabelText("Search indexed evidence") as HTMLInputElement).value).toBe("login");
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect((screen.getByLabelText("Search indexed evidence") as HTMLInputElement).value).toBe("");
    expect(screen.getByLabelText("Function, class or method name").closest(".rm-search-field")).not.toBeNull();
  });

  it("shows empty states before a search and with no saved searches, and asks for a name when saving", () => {
    render(<RetrievalSection page={makePage({ section: "retrieval" })} />);
    expect(screen.getByText("Search the evidence base").closest(".rm-empty-state")).not.toBeNull();
    expect(screen.getByText("No saved searches").closest(".rm-empty-state")).not.toBeNull();
    expect((screen.getByLabelText("Name for this search") as HTMLInputElement).required).toBe(true);
    expect((screen.getByLabelText("Question about the evidence") as HTMLTextAreaElement).required).toBe(true);
  });

  it("deletes a saved search from an icon button that names it", () => {
    saved.list = [{ id: "s1", name: "Auth flow", query: "login", artifact_types: [], languages: [], source_ids: [], result_limit: 20, shared: false, created_by: { id: "u1" } }];
    render(<RetrievalSection page={makePage({ section: "retrieval" })} />);
    expect(screen.getByRole("button", { name: "Delete Auth flow" }).className).toContain("rm-icon-button");
  });

  it("lets an owner share a private search and runs it through current search access", async () => {
    const search = { id: "s1", name: "Auth flow", query: "login", artifact_types: [], languages: [], source_ids: [], result_limit: 20, shared: false, created_by: { id: "u1" } };
    saved.list = [search];
    vi.mocked(setSharedSavedSearchSharing).mockResolvedValue({ ...search, shared: true } as never);
    vi.mocked(searchSharedWorkspace).mockResolvedValue({ results: [], semanticAvailable: false });
    render(<RetrievalSection page={makePage({ section: "retrieval" })} />);
    fireEvent.click(screen.getByRole("button", { name: "Share" }));
    await waitFor(() => expect(setSharedSavedSearchSharing).toHaveBeenCalledWith("t", "s1", true));
    fireEvent.click(screen.getByRole("button", { name: /Auth flow login/ }));
    await waitFor(() => expect(searchSharedWorkspace).toHaveBeenCalledWith("t", "w1", expect.objectContaining({ query: "login" })));
  });

  it("reopens URL filters and labels match provenance and keyword fallback", async () => {
    window.history.replaceState(null, "", "/workspaces/w1/retrieval?q=login&limit=10");
    vi.mocked(searchSharedWorkspace).mockResolvedValue({
      semanticAvailable: false,
      results: [{ artifact_id: "a1", chunk_id: "c1", title: "Login", path: "auth.md", artifact_type: "markdown_doc", language: null, snippet: "login", start_line: 3, end_line: 3, score: 1, source_name: "Docs", match_kind: "keyword", commit: null, last_author: null, last_changed_at: null }],
    });
    render(<RetrievalSection page={makePage({ section: "retrieval" })} />);
    await waitFor(() => expect(searchSharedWorkspace).toHaveBeenCalled());
    expect((screen.getByLabelText("Search indexed evidence") as HTMLInputElement).value).toBe("login");
    expect(window.location.search).toContain("limit=10");
    expect(await screen.findByText(/Keyword match: “login” · auth.md/)).toBeTruthy();
    expect(screen.getByText(/Semantic search is unavailable/)).toBeTruthy();
  });
});
