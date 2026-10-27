import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ bringBackSharedReviewItem: vi.fn(), getSharedReviewQueue: vi.fn(), snoozeSharedReviewItem: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
vi.mock("../lib/liveEvents", () => ({ useLiveRefresh: vi.fn() }));

import { bringBackSharedReviewItem, getSharedReviewQueue, snoozeSharedReviewItem } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { queryClient, workspaceKeys } from "../lib/queryClient";
import { ReviewQueuePanel, snoozeUntil } from "../components/ReviewQueuePanel";
import { parseRoute, routePath } from "../web/router";
import type { ReviewQueue } from "../types";

const queue: ReviewQueue = {
  evidence_count: 1,
  memory_card_count: 1,
  finding_count: 1,
  items: [
    { kind: "evidence", id: "a1", title: "Runbook", reason: "Steps changed in 2.1", detail: "docs/runbook.md", artifact_id: "a1", since: "2026-10-01T10:00:00Z", resolver: "writer", snoozed_until: null },
    { kind: "memory_card", id: "c1", title: "Uploads back off", reason: "Retry notes is marked outdated.", detail: "Cites 1 piece of evidence", artifact_id: "a2", since: "2026-10-02T10:00:00Z", resolver: "writer", snoozed_until: null },
    { kind: "health_finding", id: "f1", title: "Two files are identical", reason: "Copy one and Copy two have the same content.", detail: "Copy one, Copy two", artifact_id: "a3", since: null, resolver: "admin", snoozed_until: null },
  ],
  snoozed: [],
};

const handlers = { onOpenArtifact: vi.fn(), onOpenHealth: vi.fn(), onOpenMemoryCard: vi.fn() };

function panel(overrides: { canWrite?: boolean; isWorkspaceAdmin?: boolean } = {}) {
  return render(<ReviewQueuePanel accessToken="t" canWrite isWorkspaceAdmin workspaceId="w1" {...handlers} {...overrides} />);
}

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
    vi.mocked(getSharedReviewQueue).mockResolvedValue({ items: [], evidence_count: 0, memory_card_count: 0, finding_count: 0, snoozed: [] });
    panel();
    await waitFor(() => expect(screen.getByText("Nothing is waiting for review")).toBeTruthy());
    expect(screen.queryByRole("list", { name: "Review queue" })).toBeNull();
  });

  it("announces an unavailable queue and retries from the same keyboard reachable control", async () => {
    vi.mocked(getSharedReviewQueue).mockRejectedValueOnce(new Error("Connection lost")).mockResolvedValue(queue);
    panel();
    expect(screen.getByRole("status").textContent).toContain("Reading the review queue");
    expect((await screen.findByRole("alert")).textContent).toContain("Connection lost");
    const retry = screen.getByRole("button", { name: "Try again" });
    retry.focus();
    fireEvent.click(retry);
    await screen.findByText("Runbook");
    expect(getSharedReviewQueue).toHaveBeenCalledTimes(2);
    expect(showToast).not.toHaveBeenCalled();
  });

  it("snoozes an item until the day chosen, reads the list again and shares the answer with the tab", async () => {
    vi.mocked(snoozeSharedReviewItem).mockResolvedValue(undefined);
    const after: ReviewQueue = {
      ...queue, evidence_count: 0, items: queue.items.slice(1),
      snoozed: [{ ...queue.items[0], snoozed_until: "2026-11-08T00:00:00Z" }],
    };
    panel();
    await screen.findByText("Runbook");
    vi.mocked(getSharedReviewQueue).mockResolvedValue(after);
    const trigger = screen.getByRole("combobox", { name: "Snooze Runbook" });
    trigger.focus();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(await screen.findByRole("option", { name: "For a week" }), { key: "Enter" });
    await waitFor(() => expect(snoozeSharedReviewItem).toHaveBeenCalledTimes(1));
    const [token, workspaceId, item, until] = vi.mocked(snoozeSharedReviewItem).mock.calls[0];
    expect([token, workspaceId, item.kind, item.id]).toEqual(["t", "w1", "evidence", "a1"]);
    // The start of the day a week from now, as an instant.
    expect(new Date(until).getTime()).toBe(new Date(snoozeUntil(7)).getTime());
    await waitFor(() => expect(within(screen.getByRole("list", { name: "Review queue" })).queryByText("Runbook")).toBeNull());
    expect(screen.getByText(/2 items waiting/)).toBeTruthy();
    expect(screen.getByText(/1 snoozed/)).toBeTruthy();
    expect(queryClient.getQueryData(workspaceKeys.review("w1"))).toEqual(after);
    expect(showToast).toHaveBeenCalledWith("success", expect.stringContaining("Runbook is snoozed"));
    // The snoozed list says when it comes back, and brings it back at once.
    const snoozed = within(screen.getByRole("region", { name: "Snoozed items" }));
    expect(snoozed.getByText("Runbook")).toBeTruthy();
    expect(snoozed.getByText(/Back on/)).toBeTruthy();
    vi.mocked(bringBackSharedReviewItem).mockResolvedValue(undefined);
    vi.mocked(getSharedReviewQueue).mockResolvedValue(queue);
    fireEvent.click(snoozed.getByRole("button", { name: "Bring back" }));
    await waitFor(() => expect(bringBackSharedReviewItem).toHaveBeenCalledWith("t", "w1", expect.objectContaining({ kind: "evidence", id: "a1" })));
    await waitFor(() => expect(screen.getByText(/3 items waiting/)).toBeTruthy());
    expect(screen.queryByRole("region", { name: "Snoozed items" })).toBeNull();
  });

  it("says why an item could not be snoozed, and keeps it in the list", async () => {
    vi.mocked(snoozeSharedReviewItem).mockRejectedValue(new Error("You have snoozed 500 items in this workspace."));
    panel();
    await screen.findByText("Runbook");
    const trigger = screen.getByRole("combobox", { name: "Snooze Runbook" });
    trigger.focus();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(await screen.findByRole("option", { name: "Until tomorrow" }), { key: "Enter" });
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("500 items")));
    expect(screen.getByText(/3 items waiting/)).toBeTruthy();
  });

  it("counts a snooze from the start of a local day", () => {
    const now = new Date(2026, 9, 24, 21, 30);
    expect(new Date(snoozeUntil(1, now))).toEqual(new Date(2026, 9, 25, 0, 0, 0));
    expect(new Date(snoozeUntil(30, now))).toEqual(new Date(2026, 10, 23, 0, 0, 0));
    expect(new Date(snoozeUntil(1, now)).getTime()).toBeGreaterThan(now.getTime());
  });

  it("has its own address under a workspace", () => {
    expect(parseRoute("/workspaces/w1/review")).toEqual({ name: "workspace", workspaceId: "w1", section: "review" });
    expect(routePath({ name: "workspace", workspaceId: "w1", section: "review" })).toBe("/workspaces/w1/review");
  });
});
