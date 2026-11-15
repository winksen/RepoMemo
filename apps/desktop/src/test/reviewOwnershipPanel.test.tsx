import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  completeSharedReview: vi.fn(),
  getSharedReviewAssignment: vi.fn(),
  listSharedWorkspaceMembers: vi.fn(),
  setSharedReviewAssignment: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { getSharedReviewAssignment, listSharedWorkspaceMembers, setSharedReviewAssignment } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { ReviewOwnershipPanel } from "../components/ReviewOwnershipPanel";
import type { ReviewAssignmentView } from "../types";

const assignment = (overrides: Partial<ReviewAssignmentView> = {}): ReviewAssignmentView => ({
  kind: "memory_card", item_id: "c1", reviewer: null, reviewer_departed: false, reviewer_label: null, next_review_at: null, last_reviewed_at: null, overdue: false, ...overrides,
});

function renderPanel(canWrite = true) {
  return render(<ReviewOwnershipPanel accessToken="t" canWrite={canWrite} itemId="c1" kind="memory_card" workspaceId="w1" />);
}

describe("the reviewer panel on its states", () => {
  beforeEach(() => { vi.clearAllMocks(); vi.mocked(listSharedWorkspaceMembers).mockResolvedValue([]); });
  afterEach(cleanup);

  it("shows nothing until the assignment is read", () => {
    vi.mocked(getSharedReviewAssignment).mockReturnValue(new Promise(() => undefined));
    const { container } = renderPanel();
    expect(container.textContent).toBe("");
  });

  it("says nobody is accountable yet, which is the empty state", async () => {
    vi.mocked(getSharedReviewAssignment).mockResolvedValue(assignment());
    renderPanel();
    expect(await screen.findByText(/Nobody is accountable for reviewing this yet/)).toBeTruthy();
    expect(screen.getByText("No review date")).toBeTruthy();
  });

  it("names a reviewer who left, so nobody is mistaken for accountable", async () => {
    vi.mocked(getSharedReviewAssignment).mockResolvedValue(assignment({ reviewer_departed: true, reviewer_label: "Ana" }));
    renderPanel();
    expect(await screen.findByText(/has left this workspace, so nobody is accountable/)).toBeTruthy();
  });

  it("marks an overdue review as overdue", async () => {
    vi.mocked(getSharedReviewAssignment).mockResolvedValue(assignment({ overdue: true, next_review_at: "2026-10-01T00:00:00+00:00" }));
    renderPanel();
    expect(await screen.findByText("Review is overdue")).toBeTruthy();
  });

  it("gives a reader the facts without the form", async () => {
    vi.mocked(getSharedReviewAssignment).mockResolvedValue(assignment({ next_review_at: "2026-12-01T00:00:00+00:00" }));
    renderPanel(false);
    expect(await screen.findByText(/^Next review /)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    expect(listSharedWorkspaceMembers).not.toHaveBeenCalled();
  });

  it("reports a refused read in a toast", async () => {
    vi.mocked(getSharedReviewAssignment).mockRejectedValue(new Error("You do not have access to this workspace."));
    renderPanel();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "You do not have access to this workspace."));
  });

  it("reports a refused save, such as a reviewer who cannot edit, and keeps the form", async () => {
    vi.mocked(getSharedReviewAssignment).mockResolvedValue(assignment());
    vi.mocked(setSharedReviewAssignment).mockRejectedValue(new Error("A reviewer must be a member who can edit."));
    renderPanel();
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "A reviewer must be a member who can edit."));
    expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
  });
});
