import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  SharedApiError: class SharedApiError extends Error { status = 0; },
  createSharedInvitation: vi.fn(),
  listSharedInvitations: vi.fn(),
  listSharedOrganizationMembers: vi.fn(),
  removeSharedOrganizationMember: vi.fn(),
  revokeSharedInvitation: vi.fn(),
  transferSharedOrganizationOwnership: vi.fn(),
  updateSharedOrganization: vi.fn(),
  upsertSharedOrganizationMember: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));
vi.mock("../components/UserAvatar", () => ({ UserAvatar: () => null }));

import { listSharedInvitations, listSharedOrganizationMembers } from "../lib/sharedApi";
import { InviteMembers } from "../components/InviteMembers";
import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { OrganizationAdministration } from "../web/Organization";
import type { Organization, OrganizationMember } from "../types";

const organization = { id: "o1", name: "Acme", role: "owner" } as Organization;
const owner = { role: "owner", user: { id: "u1", display_name: "Grace", email: "grace@example.com" } } as OrganizationMember;

beforeEach(() => { vi.clearAllMocks(); });
afterEach(() => { cleanup(); });

describe("organization administration on the shared pieces", () => {
  it("shows the loading state, then the owner as a badge, and labels the rename and member fields", async () => {
    vi.mocked(listSharedOrganizationMembers).mockResolvedValue([owner]);
    vi.mocked(listSharedInvitations).mockResolvedValue([]);
    render(<ConfirmProvider><OrganizationAdministration accessToken="t" onOrganizationUpdated={vi.fn()} organization={organization} /></ConfirmProvider>);
    expect(screen.getByRole("status").textContent).toBe("Loading organization members…");
    const owned = await screen.findByText("Owner");
    expect(owned.className).toContain("rm-badge");
    const name = screen.getByLabelText("Organization name") as HTMLInputElement;
    expect(name.required).toBe(true);
    expect(name.closest(".rm-field")).not.toBeNull();
    expect((screen.getByLabelText("Member email") as HTMLInputElement).required).toBe(true);
    expect(screen.getByLabelText("Organization role").closest(".rm-field")).not.toBeNull();
  });

  it("keeps Save name off until the name changes", async () => {
    vi.mocked(listSharedOrganizationMembers).mockResolvedValue([owner]);
    vi.mocked(listSharedInvitations).mockResolvedValue([]);
    render(<ConfirmProvider><OrganizationAdministration accessToken="t" onOrganizationUpdated={vi.fn()} organization={organization} /></ConfirmProvider>);
    await screen.findByText("Owner");
    expect((screen.getByRole("button", { name: "Save name" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Organization name"), { target: { value: "Acme Engineering" } });
    expect((screen.getByRole("button", { name: "Save name" }) as HTMLButtonElement).disabled).toBe(false);
  });
});

describe("the invitation form on the shared pieces", () => {
  it("labels the address and the role and keeps the button off until there is an address", async () => {
    vi.mocked(listSharedInvitations).mockResolvedValue([]);
    render(<InviteMembers accessToken="t" roleOptions={[{ label: "Member", value: "member" }]} targetId="o1" targetLabel="organization" targetType="organization" />);
    const email = await screen.findByLabelText("Email to invite") as HTMLInputElement;
    expect(email.required).toBe(true);
    expect(email.closest(".rm-field")).not.toBeNull();
    expect(screen.getByLabelText("Role for the invited person").closest(".rm-field")).not.toBeNull();
    expect((screen.getByRole("button", { name: /Create invite link/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(email, { target: { value: "ada@example.com" } });
    expect((screen.getByRole("button", { name: /Create invite link/ }) as HTMLButtonElement).disabled).toBe(false);
  });
});
