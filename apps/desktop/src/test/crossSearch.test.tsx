import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ searchAllWorkspaces: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
vi.mock("../web/router", async (original) => ({ ...(await original<typeof import("../web/router")>()), navigate: vi.fn() }));

import { searchAllWorkspaces } from "../lib/sharedApi";
import { navigate } from "../web/router";
import { CrossWorkspaceSearch } from "../web/CrossSearch";
import type { CrossSearchResponse } from "../types";

const found: CrossSearchResponse = {
  results: [{
    workspace_id: "w1", workspace_name: "Platform",
    result: { chunk_id: "c1", artifact_id: "a1", title: "Login flow", artifact_type: "markdown_doc", source_name: "Pasted notes", path: "login.md", snippet: "A <mark>session</mark> ends", start_line: 2, score: 1 } as never,
  }],
  memory_cards: [{
    workspace_id: "w2", workspace_name: "Billing",
    card: { id: "m1", workspace_id: "w2", title: "Session rule", body_excerpt: "A session ends after a week.", source: "manual", evidence_count: 0, created_at: "2026-10-01T10:00:00Z", updated_at: "2026-10-02T10:00:00Z", needs_review: true },
  }],
  workspaces: [
    { workspace_id: "w1", name: "Platform", result_count: 1, by_meaning: false, note: null },
    { workspace_id: "w2", name: "Billing", result_count: 0, by_meaning: false, note: null },
  ],
  not_searched: 0,
};

// Radix measures and scrolls; jsdom has neither.
beforeAll(() => {
  Object.assign(window.HTMLElement.prototype, {
    hasPointerCapture: () => false,
    releasePointerCapture: () => undefined,
    scrollIntoView: () => undefined,
    setPointerCapture: () => undefined,
  });
  window.ResizeObserver = window.ResizeObserver ?? class { disconnect() { /* none */ } observe() { /* none */ } unobserve() { /* none */ } };
});

beforeEach(() => { vi.mocked(searchAllWorkspaces).mockResolvedValue(found); });
afterEach(cleanup);

function ask(text: string) {
  fireEvent.change(screen.getByLabelText("Search every workspace you can read"), { target: { value: text } });
  fireEvent.click(screen.getByRole("button", { name: /^Search$/ }));
}

describe("CrossWorkspaceSearch", () => {
  it("searches files and memory cards with no filter by default, and says what it found", async () => {
    render(<CrossWorkspaceSearch accessToken="t" />);
    ask("session");
    await screen.findByText("Login flow");
    expect(searchAllWorkspaces).toHaveBeenCalledWith("t", { query: "session", artifactTypes: [], languages: [], modifiedAfter: "", modifiedBefore: "", memoryCards: true });
    expect(screen.getByText(/1 result and 1 memory card for “session” in 2 workspaces/)).toBeTruthy();
    const cards = within(screen.getByRole("list", { name: "Memory cards found" }));
    expect(cards.getByText("Session rule")).toBeTruthy();
    expect(cards.getByText(/Billing · Memory card · Needs review/)).toBeTruthy();
  });

  it("opens a file and a memory card in their workspace", async () => {
    render(<CrossWorkspaceSearch accessToken="t" />);
    ask("session");
    fireEvent.click(await screen.findByRole("button", { name: /Login flow/ }));
    expect(navigate).toHaveBeenCalledWith("/workspaces/w1/artifacts/a1");
    fireEvent.click(screen.getByRole("button", { name: /Session rule/ }));
    expect(navigate).toHaveBeenLastCalledWith("/workspaces/w2/memory-cards/m1");
  });

  it("sends the language and the days chosen", async () => {
    render(<CrossWorkspaceSearch accessToken="t" />);
    fireEvent.change(screen.getByLabelText("Language"), { target: { value: " Rust " } });
    fireEvent.change(screen.getByLabelText("Updated from"), { target: { value: "2026-10-01" } });
    fireEvent.change(screen.getByLabelText("Updated to"), { target: { value: "2026-10-31" } });
    ask("session");
    await waitFor(() => expect(searchAllWorkspaces).toHaveBeenCalledTimes(1));
    expect(searchAllWorkspaces).toHaveBeenCalledWith("t", { query: "session", artifactTypes: [], languages: ["Rust"], modifiedAfter: "2026-10-01", modifiedBefore: "2026-10-31", memoryCards: true });
  });

  it("leaves memory cards out when a language is chosen, or when they are turned off", async () => {
    render(<CrossWorkspaceSearch accessToken="t" />);
    const box = screen.getByRole("checkbox", { name: /Include memory cards/ });
    expect(box.getAttribute("aria-checked")).toBe("true");
    fireEvent.change(screen.getByLabelText("Language"), { target: { value: "Rust" } });
    expect(box.getAttribute("aria-checked")).toBe("false");
    expect(box.hasAttribute("disabled") || box.getAttribute("data-disabled") !== null).toBe(true);
    expect(screen.getByText(/Cards are not files/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Language"), { target: { value: "" } });
    fireEvent.click(box);
    ask("session");
    await waitFor(() => expect(searchAllWorkspaces).toHaveBeenCalledTimes(1));
    expect(vi.mocked(searchAllWorkspaces).mock.calls[0][1].memoryCards).toBe(false);
  });
});
