import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ getSharedOnboarding: vi.fn(), setSharedOnboardingDismissed: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
vi.mock("../lib/liveEvents", () => ({ useLiveRefresh: vi.fn() }));

import { getSharedOnboarding, setSharedOnboardingDismissed } from "../lib/sharedApi";
import { OnboardingChecklist } from "../components/OnboardingChecklist";
import type { WorkspaceOnboarding } from "../types";

const onboarding = (overrides: Partial<WorkspaceOnboarding> = {}): WorkspaceOnboarding => ({
  steps: [
    { id: "add_evidence", title: "Add your first piece of evidence", detail: "Upload a file.", done: true },
    { id: "searchable", title: "Make it searchable", detail: "1 of 1 indexed.", done: true },
    { id: "cited_memory", title: "Save a memory card that cites it", detail: "Keep a conclusion.", done: false },
    { id: "invite", title: "Bring in a teammate", detail: "Optional.", done: false },
  ],
  done_count: 2,
  total_count: 4,
  dismissed: false,
  template_name: "General",
  ...overrides,
});

describe("OnboardingChecklist", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(cleanup);

  it("shows the steps with what is done, taken from the workspace rather than from clicks", async () => {
    vi.mocked(getSharedOnboarding).mockResolvedValue(onboarding());
    render(<OnboardingChecklist accessToken="t" workspaceId="w1" />);
    expect(await screen.findByText("Save a memory card that cites it")).toBeTruthy();
    expect(screen.getByText("2 of 4 done")).toBeTruthy();
    expect(screen.getAllByText("Done")).toHaveLength(2);
    expect(screen.getAllByText("To do")).toHaveLength(2);
  });

  it("can be hidden, and shown again from a single button", async () => {
    vi.mocked(getSharedOnboarding).mockResolvedValue(onboarding());
    vi.mocked(setSharedOnboardingDismissed).mockResolvedValue(undefined);
    render(<OnboardingChecklist accessToken="t" workspaceId="w1" />);
    fireEvent.click(await screen.findByRole("button", { name: "Hide for now" }));
    await waitFor(() => expect(setSharedOnboardingDismissed).toHaveBeenCalledWith("t", "w1", true));
    fireEvent.click(await screen.findByRole("button", { name: /Show the getting-started checklist \(2 of 4\)/ }));
    await waitFor(() => expect(setSharedOnboardingDismissed).toHaveBeenLastCalledWith("t", "w1", false));
    expect(await screen.findByText("Make it searchable")).toBeTruthy();
  });

  it("stays out of the way once finished and dismissed", async () => {
    vi.mocked(getSharedOnboarding).mockResolvedValue(onboarding({ dismissed: true, done_count: 4, steps: onboarding().steps.map((step) => ({ ...step, done: true })) }));
    const { container } = render(<OnboardingChecklist accessToken="t" workspaceId="w1" />);
    await waitFor(() => expect(getSharedOnboarding).toHaveBeenCalled());
    expect(container.textContent).toBe("");
  });
});
