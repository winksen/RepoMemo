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
import { EvidenceProvenance, IndexedChunksDialog } from "../web/ArtifactDetail";
import type { ArtifactDetail, ArtifactLifecycle, ArtifactSummary } from "../types";

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

describe("evidence provenance", () => {
  it("shows source sync, current index and review state together", () => {
    const detail = {
      summary: { ...artifact, source_id: "s1", source_name: "Engineering repo", created_at: "2026-10-01T00:00:00Z", updated_at: "2026-10-02T00:00:00Z", indexed_at: "2026-10-02T01:00:00Z" },
      index_version: 4, source_synced_at: "2026-10-03T00:00:00Z",
    } as ArtifactDetail;
    render(<EvidenceProvenance artifact={detail} lifecycle={{ status: "needs_review", reviewed_at: null } as ArtifactLifecycle} />);
    expect(screen.getByText("Engineering repo", { exact: false })).toBeTruthy();
    expect(screen.getByText("Source last synced")).toBeTruthy();
    expect(screen.getByText("Index version").nextElementSibling?.textContent).toBe("4");
    expect(screen.getByText("Review state").nextElementSibling?.textContent).toBe("needs review");
  });

  it("makes the absence of a current index explicit", () => {
    const detail = { summary: { ...artifact, source_id: "s1", source_name: "Uploads", created_at: "2026-10-01T00:00:00Z", updated_at: "2026-10-02T00:00:00Z", indexed_at: null }, index_version: 3, source_synced_at: null } as ArtifactDetail;
    render(<EvidenceProvenance artifact={detail} lifecycle={null} />);
    expect(screen.getByText("Imported")).toBeTruthy();
    expect(screen.getByText("No index for current content")).toBeTruthy();
    expect(screen.getByText("Index version").nextElementSibling?.textContent).toBe("Not indexed");
  });
});
