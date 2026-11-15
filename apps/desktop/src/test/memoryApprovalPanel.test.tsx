import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  approveSharedMemoryCard: vi.fn(),
  getSharedApprovalPolicy: vi.fn(),
  getSharedMemoryApproval: vi.fn(),
  setSharedApprovalPolicy: vi.fn(),
  withdrawSharedMemoryApproval: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { approveSharedMemoryCard, getSharedMemoryApproval, setSharedApprovalPolicy } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { MemoryApprovalPanel } from "../components/MemoryApprovalPanel";
import type { MemoryApprovalStatus } from "../types";

const status = (overrides: Partial<MemoryApprovalStatus> = {}): MemoryApprovalStatus => ({
  revision: 2, approved: false, approved_by: null, approved_at: null, independent_review: false, can_approve: true, cannot_approve_reason: null, history: [], imported: [], ...overrides,
});

function renderPanel(isAdmin = false) {
  return render(<MemoryApprovalPanel accessToken="t" cardId="c1" isAdmin={isAdmin} updatedAt="now" workspaceId="w1" />);
}

describe("the approval panel on its states", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(cleanup);

  it("shows nothing until the status is read", () => {
    vi.mocked(getSharedMemoryApproval).mockReturnValue(new Promise(() => undefined));
    const { container } = renderPanel();
    expect(container.textContent).toBe("");
  });

  it("offers to approve the revision the reviewer read, and shows the result", async () => {
    vi.mocked(getSharedMemoryApproval).mockResolvedValue(status());
    vi.mocked(approveSharedMemoryCard).mockResolvedValue(status({ approved: true, can_approve: false, cannot_approve_reason: "This revision is already approved." }));
    renderPanel();
    expect(await screen.findByText("Revision 2 has not been approved.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Approve revision 2" }));
    await waitFor(() => expect(approveSharedMemoryCard).toHaveBeenCalledWith("t", "c1", 2));
    expect(await screen.findByText("Approved")).toBeTruthy();
  });

  it("says in a toast when the card moved past the revision that was read, and offers nothing wrong", async () => {
    vi.mocked(getSharedMemoryApproval).mockResolvedValue(status());
    vi.mocked(approveSharedMemoryCard).mockRejectedValue(new Error("This card was saved again (revision 3). Read it before approving."));
    renderPanel();
    fireEvent.click(await screen.findByRole("button", { name: "Approve revision 2" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "This card was saved again (revision 3). Read it before approving."));
  });

  it("explains a refusal in words and offers no approve button", async () => {
    vi.mocked(getSharedMemoryApproval).mockResolvedValue(status({ can_approve: false, independent_review: true, cannot_approve_reason: "This workspace asks for independent review: you made this revision, so someone else must approve it." }));
    renderPanel();
    expect(await screen.findByText(/you made this revision, so someone else must approve it/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Approve revision/ })).toBeNull();
  });

  it("keeps a decision from another server as a record that approves nothing here", async () => {
    vi.mocked(getSharedMemoryApproval).mockResolvedValue(status({ imported: [{ revision: 2, decision: "approved", person: "Ana <ana@example.com>", note: "Checked", decided_at: "2026-10-01T00:00:00+00:00" }] }));
    renderPanel();
    expect(await screen.findByText(/nothing is approved here until someone here approves it/)).toBeTruthy();
    expect(screen.getByText("Approved elsewhere · revision 2")).toBeTruthy();
    expect(screen.getByText("Revision 2 has not been approved.")).toBeTruthy();
  });

  it("lets only an administrator change the policy, and reports a refusal", async () => {
    vi.mocked(getSharedMemoryApproval).mockResolvedValue(status());
    vi.mocked(setSharedApprovalPolicy).mockRejectedValue(new Error("Only an owner or administrator can change this."));
    renderPanel(true);
    fireEvent.click(await screen.findByRole("switch"));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "Only an owner or administrator can change this."));
    cleanup();
    vi.mocked(getSharedMemoryApproval).mockResolvedValue(status());
    renderPanel(false);
    await screen.findByText("Revision 2 has not been approved.");
    expect(screen.queryByRole("switch")).toBeNull();
  });

  it("reports a failed read in a toast", async () => {
    vi.mocked(getSharedMemoryApproval).mockRejectedValue(new Error("The approval could not be read."));
    renderPanel();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "The approval could not be read."));
  });
});
