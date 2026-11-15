import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ getSharedMyWork: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { getSharedMyWork } from "../lib/sharedApi";
import { queryClient } from "../lib/queryClient";
import { showToast } from "../components/ui/toast";
import { MyWorkPanel } from "../components/MyWorkPanel";
import type { CollaborationTask, MyWork } from "../types";

const work = (overrides: Partial<MyWork> = {}): MyWork => ({ tasks: [], reviews: [], mentions: [], overdue_task_count: 0, total_count: 0, ...overrides });
const task = { id: "t1", workspace_id: "w1", title: "Check the refund policy", due_at: null } as unknown as CollaborationTask;

function renderPanel() {
  return render(<QueryClientProvider client={queryClient}><MyWorkPanel accessToken="t" userId="u1" /></QueryClientProvider>);
}

describe("My work on its shared states", () => {
  beforeEach(() => { queryClient.clear(); vi.clearAllMocks(); });
  afterEach(cleanup);

  it("shows nothing while the first answer is on its way", () => {
    vi.mocked(getSharedMyWork).mockReturnValue(new Promise(() => undefined));
    const { container } = renderPanel();
    expect(container.textContent).toBe("");
  });

  it("says plainly that nothing waits when the server reports nothing", async () => {
    vi.mocked(getSharedMyWork).mockResolvedValue(work());
    renderPanel();
    expect(await screen.findByText(/Nothing is waiting for you/)).toBeTruthy();
  });

  it("lists what waits and counts the overdue tasks", async () => {
    vi.mocked(getSharedMyWork).mockResolvedValue(work({ tasks: [task], overdue_task_count: 1, total_count: 1 }));
    renderPanel();
    expect(await screen.findByText("Check the refund policy")).toBeTruthy();
    expect(screen.getByText("1 waiting, 1 overdue")).toBeTruthy();
  });

  it("does not call a failed read an empty list: it says it could not read, in a toast", async () => {
    vi.mocked(getSharedMyWork).mockRejectedValue(new Error("The server is unreachable."));
    const { container } = renderPanel();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "The server is unreachable."));
    expect(container.textContent).not.toContain("Nothing is waiting");
  });
});
