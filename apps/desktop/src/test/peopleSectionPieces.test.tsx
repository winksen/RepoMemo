import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  SharedApiError: class SharedApiError extends Error { status = 0; },
  createSharedInvitation: vi.fn(),
  listSharedInvitations: vi.fn().mockResolvedValue([]),
  removeSharedWorkspaceMember: vi.fn(),
  revokeSharedInvitation: vi.fn(),
  transferSharedWorkspaceOwnership: vi.fn(),
  upsertSharedWorkspaceMember: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));
vi.mock("../components/UserAvatar", () => ({ UserAvatar: () => null }));
vi.mock("../web/workspace/data", () => ({
  useActivity: () => [[]],
  useActivityCalendar: () => [null],
  useCapabilities: () => [null],
  useMembers: () => [[{ role: "admin", user: { id: "u2", display_name: "Grace", email: "grace@example.com" } }]],
}));
vi.mock("../web/common", () => ({ ContributionCalendar: () => null }));

import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { PeopleSection } from "../web/workspace/PeopleSection";
import type { WorkspacePage } from "../web/workspace/page";

afterEach(() => { cleanup(); });

function renderPeople() {
  const state = new Map<string, unknown>();
  const page = {
    accessToken: "t",
    canAssignAdmin: true,
    canManageMembers: true,
    canManageWorkspace: false,
    canWrite: true,
    isSubmitting: false,
    load: vi.fn(),
    session: { user: { id: "u1" } },
    setError: vi.fn(),
    setIsSubmitting: vi.fn(),
    store: state,
    workspace: { role: "admin", workspace: { id: "w1", name: "Atlas" } },
    workspaceId: "w1",
  } as unknown as WorkspacePage;
  return render(<ConfirmProvider><PeopleSection page={page} /></ConfirmProvider>);
}

describe("the people section on the shared pieces", () => {
  it("shows each member's role as a badge and labels the member form's fields", () => {
    renderPeople();
    expect(screen.getByText("admin").className).toContain("rm-badge");
    const email = screen.getByLabelText("Member email") as HTMLInputElement;
    expect(email.required).toBe(true);
    expect(email.closest(".rm-field")).not.toBeNull();
    expect(screen.getByLabelText("Member role").closest(".rm-field")).not.toBeNull();
  });

  it("shows the shared empty state when there is no activity", () => {
    renderPeople();
    expect(screen.getByText("No recorded activity yet").closest(".rm-empty-state")).not.toBeNull();
  });
});
