import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  indexSharedArtifact: vi.fn(),
  listSharedArtifactChunks: vi.fn(),
  waitForSharedJob: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { listSharedArtifactChunks } from "../lib/sharedApi";
import { queryClient } from "../lib/queryClient";
import { IndexedChunksDialog } from "../web/ArtifactDetail";
import type { ArtifactSummary } from "../types";

const artifact = { id: "a1", workspace_id: "w1", title: "Runbook", path: "docs/runbook.md" } as ArtifactSummary;

beforeAll(() => {
  // jsdom does not implement the modal dialog.
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) { this.setAttribute("open", ""); };
});
beforeEach(() => { queryClient.clear(); });
afterEach(() => { cleanup(); });

function renderDialog() {
  return render(<QueryClientProvider client={queryClient}><IndexedChunksDialog accessToken="t" artifact={artifact} onClose={vi.fn()} onReindexed={vi.fn()} /></QueryClientProvider>);
}

describe("the indexed chunks dialog on the shared states", () => {
  it("shows the shared loading state while the chunks are read", () => {
    vi.mocked(listSharedArtifactChunks).mockReturnValue(new Promise(() => undefined));
    renderDialog();
    expect(screen.getByRole("status").textContent).toBe("Loading chunks…");
  });

  it("shows the shared empty state when nothing is stored", async () => {
    vi.mocked(listSharedArtifactChunks).mockResolvedValue([]);
    renderDialog();
    expect((await screen.findByText("No chunks are stored for this artifact")).closest(".rm-empty-state")).not.toBeNull();
  });
});
