import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ getSharedReviewQueue: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
vi.mock("../lib/liveEvents", () => ({ useLiveRefresh: vi.fn() }));

import { getSharedReviewQueue } from "../lib/sharedApi";
import { ReviewQueuePanel } from "../components/ReviewQueuePanel";
import { parseRoute, routePath } from "../web/router";
import type { ReviewQueue } from "../types";

const queue: ReviewQueue = {
  evidence_count: 1,
  memory_card_count: 1,
  finding_count: 1,
  items: [
    { kind: "evidence", id: "a1", title: "Runbook", reason: "Steps changed in 2.1", detail: "docs/runbook.md", artifact_id: "a1", since: "2026-10-01T10:00:00Z", resolver: "writer" },
    { kind: "memory_card", id: "c1", title: "Uploads back off", reason: "Retry notes is marked outdated.", detail: "Cites 1 piece of evidence", artifact_id: "a2", since: "2026-10-02T10:00:00Z", resolver: "writer" },
    { kind: "health_finding", id: "f1", title: "Two files are identical", reason: "Copy one and Copy two have the same content.", detail: "Copy one, Copy two", artifact_id: "a3", since: null, resolver: "admin" },
  ],
};

const handlers = { onOpenArtifact: vi.fn(), onOpenHealth: vi.fn(), onOpenMemoryCard: vi.fn() };

function panel(overrides: { canWrite?: boolean; isWorkspaceAdmin?: boolean } = {}) {
  return render(<ReviewQueuePanel accessToken="t" canWrite isWorkspaceAdmin workspaceId="w1" {...handlers} {...overrides} />);
}

describe("ReviewQueuePanel", () => {
  beforeEach(() => {
    vi.mocked(getSharedReviewQueue).mockResolvedValue(queue);
    Object.values(handlers).forEach((handler) => handler.mockClear());
  });
  afterEach(() => cleanup());

  it("lists every kind of item and opens each where it is resolved", async () => {
    panel();
    await screen.findByText("Runbook");
    expect(screen.getByText(/3 items waiting/)).toBeTruthy();
    const open = screen.getAllByRole("button", { name: /^Open / });
    expect(open.map((button) => button.textContent)).toEqual(["Open file", "Open card", "Open in Health"]);
    fireEvent.click(open[0]);
    expect(handlers.onOpenArtifact).toHaveBeenCalledWith("a1");
    fireEvent.click(open[1]);
    expect(handlers.onOpenMemoryCard).toHaveBeenCalledWith("c1");
    fireEvent.click(open[2]);
    expect(handlers.onOpenHealth).toHaveBeenCalled();
  });

  it("says what a person cannot resolve themselves", async () => {
    panel({ canWrite: false, isWorkspaceAdmin: false });
    await screen.findByText("Runbook");
    expect(screen.getAllByText(/someone who can edit the workspace resolves it/).length).toBe(2);
    expect(screen.getByText(/Only an owner or administrator can resolve this/)).toBeTruthy();
  });

  it("shows an empty queue as nothing waiting", async () => {
    vi.mocked(getSharedReviewQueue).mockResolvedValue({ items: [], evidence_count: 0, memory_card_count: 0, finding_count: 0 });
    panel();
    await waitFor(() => expect(screen.getByText("Nothing is waiting for review")).toBeTruthy());
    expect(screen.queryByRole("list", { name: "Review queue" })).toBeNull();
  });

  it("has its own address under a workspace", () => {
    expect(parseRoute("/workspaces/w1/review")).toEqual({ name: "workspace", workspaceId: "w1", section: "review" });
    expect(routePath({ name: "workspace", workspaceId: "w1", section: "review" })).toBe("/workspaces/w1/review");
  });
});
