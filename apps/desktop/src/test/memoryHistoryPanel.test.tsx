import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/sharedApi")>()),
  compareSharedMemoryRevisions: vi.fn(),
  getSharedCitedSource: vi.fn(),
  listSharedMemoryRevisions: vi.fn(),
  restoreSharedMemoryRevision: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { getSharedCitedSource, listSharedMemoryRevisions, restoreSharedMemoryRevision, SharedApiError } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { MemoryHistoryPanel } from "../components/MemoryHistoryPanel";
import type { MemoryRevision, MemoryRevisionCitation } from "../types";

const citation = (overrides: Partial<MemoryRevisionCitation> = {}): MemoryRevisionCitation => ({
  target_type: "artifact", target_id: "a1", title: "Refund policy", path: "refund.md", confidence: null, artifact_id: "a1", content_hash: "0123456789abcdef", currency: "changed", missing_at_save: false, ...overrides,
});
const revision = (number: number, overrides: Partial<MemoryRevision> = {}): MemoryRevision => ({
  revision: number, title: "Refund window", body_markdown: `Edit ${number}.`, source: "manual", confidence: null, citations: [], author: { id: "u1", display_name: "Ana", email: null }, author_label: null, created_at: "2026-11-01T10:00:00+00:00", restored_from: null, ...overrides,
});

beforeAll(() => {
  // jsdom does not implement the modal dialog.
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) { this.setAttribute("open", ""); };
});
afterAll(() => { delete (HTMLDialogElement.prototype as Partial<HTMLDialogElement>).showModal; });

function renderPanel(canWrite = true) {
  return render(<ConfirmProvider><MemoryHistoryPanel accessToken="t" canWrite={canWrite} cardId="c1" onRestored={vi.fn()} updatedAt="now" /></ConfirmProvider>);
}

describe("the revision history panel on its states", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(cleanup);

  it("shows nothing for a card with a single revision, and nothing while loading", async () => {
    vi.mocked(listSharedMemoryRevisions).mockResolvedValue({ items: [revision(1)], nextBefore: null, total: 1 });
    const { container } = renderPanel();
    expect(container.textContent).toBe("");
    await waitFor(() => expect(listSharedMemoryRevisions).toHaveBeenCalled());
    expect(container.textContent).toBe("");
  });

  it("lists revisions newest first with who saved them and which one is current", async () => {
    vi.mocked(listSharedMemoryRevisions).mockResolvedValue({ items: [revision(2), revision(1, { author: null, author_label: "Bo <bo@example.com>" })], nextBefore: null, total: 2 });
    renderPanel();
    expect(await screen.findByText("Revision 2 · current")).toBeTruthy();
    expect(screen.getByText(/Saved by Ana/)).toBeTruthy();
    expect(screen.getByText(/Saved by Bo <bo@example.com> \(on the server it was imported from\)/)).toBeTruthy();
  });

  it("pages a long history and appends the earlier revisions on request", async () => {
    vi.mocked(listSharedMemoryRevisions)
      .mockResolvedValueOnce({ items: [revision(3), revision(2)], nextBefore: 2, total: 3 })
      .mockResolvedValueOnce({ items: [revision(1)], nextBefore: null, total: 3 });
    renderPanel();
    expect(await screen.findByText("2 of 3 revisions")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Show earlier revisions" }));
    expect(await screen.findByText("Revision 1")).toBeTruthy();
    expect(listSharedMemoryRevisions).toHaveBeenLastCalledWith("t", "c1", 2);
    expect(screen.queryByRole("button", { name: "Show earlier revisions" })).toBeNull();
  });

  it("offers no restore to a reader", async () => {
    vi.mocked(listSharedMemoryRevisions).mockResolvedValue({ items: [revision(2), revision(1)], nextBefore: null, total: 2 });
    renderPanel(false);
    await screen.findByText("Revision 2 · current");
    expect(screen.queryByRole("button", { name: /Restore revision/ })).toBeNull();
  });

  it("asks before restoring, and reports a refused restore without changing the list", async () => {
    vi.mocked(listSharedMemoryRevisions).mockResolvedValue({ items: [revision(2), revision(1)], nextBefore: null, total: 2 });
    vi.mocked(restoreSharedMemoryRevision).mockRejectedValue(new Error("That revision was not found."));
    renderPanel();
    fireEvent.click(await screen.findByRole("button", { name: "Restore revision 1" }));
    fireEvent.click(await screen.findByRole("button", { name: "Restore" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "That revision was not found."));
    expect(restoreSharedMemoryRevision).toHaveBeenCalledWith("t", "c1", 1);
  });

  it("says a historical citation is changed and, when access is refused, that the sources cannot be shown", async () => {
    vi.mocked(listSharedMemoryRevisions).mockResolvedValue({ items: [revision(2, { citations: [citation()] }), revision(1)], nextBefore: null, total: 2 });
    vi.mocked(getSharedCitedSource).mockRejectedValue(new SharedApiError(403, "Forbidden"));
    renderPanel();
    fireEvent.click(await screen.findByRole("button", { name: "Sources" }));
    expect(await screen.findByText(/the file has changed since/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "View cited text" }));
    expect(await screen.findByText("You no longer have access to this card, so its sources cannot be shown.")).toBeTruthy();
  });

  it("reports a failed read in a toast", async () => {
    vi.mocked(listSharedMemoryRevisions).mockRejectedValue(new Error("The history could not be read."));
    renderPanel();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "The history could not be read."));
  });
});
