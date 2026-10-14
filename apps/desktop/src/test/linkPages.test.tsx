import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  acceptSharedInvitation: vi.fn(),
  completeSharedPasswordReset: vi.fn(),
  loginSharedUser: vi.fn(),
  previewSharedInvitation: vi.fn(),
  previewSharedPasswordReset: vi.fn(),
  registerSharedUser: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
vi.mock("../components/SharedLayout", () => ({ ThemeToggle: () => null }));

import {
  acceptSharedInvitation, completeSharedPasswordReset, loginSharedUser, previewSharedInvitation,
  previewSharedPasswordReset, registerSharedUser,
} from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { InvitePage, invitationDestination } from "../components/InvitePage";
import { PasswordResetPage } from "../components/PasswordResetPage";
import type { InvitationPreview, PasswordResetPreview, SharedSession } from "../types";

const invitation = (overrides: Partial<InvitationPreview> = {}): InvitationPreview => ({
  email: "ada@example.com",
  target_type: "workspace",
  target_id: "w1",
  target_name: "Atlas",
  organization_name: "Acme",
  role: "member",
  invited_by: "Grace",
  expires_at: "2099-01-01T00:00:00Z",
  account_exists: true,
  ...overrides,
});

const sessionFor = (email: string): SharedSession => ({
  user: { id: "u1", email, display_name: "Someone" },
  authentication: "jwt",
  memberships: [],
}) as SharedSession;

const accepted = { target_type: "workspace" as const, target_id: "w1", organization_id: "o1", role: "member" };

function renderInvite(overrides: Partial<ComponentProps<typeof InvitePage>> = {}) {
  const props = {
    accessToken: null,
    onAuthenticated: vi.fn(),
    onJoined: vi.fn(),
    onSignOut: vi.fn(),
    session: null,
    token: "secret",
    ...overrides,
  };
  render(<InvitePage {...props} />);
  return props;
}

beforeEach(() => { vi.clearAllMocks(); });
afterEach(() => { cleanup(); });

describe("invitation page", () => {
  it("says what the invitation is for while it is being checked, then who, where and until when", async () => {
    vi.mocked(previewSharedInvitation).mockResolvedValue(invitation());
    renderInvite();
    expect(screen.getByText("Checking the invitation…")).toBeTruthy();
    await screen.findByRole("heading", { name: "Join Atlas" });
    expect(screen.getByText(/Atlas · in Acme/)).toBeTruthy();
    expect(screen.getByText("ada@example.com")).toBeTruthy();
    expect(previewSharedInvitation).toHaveBeenCalledWith("secret");
  });

  it("asks someone with an account to sign in, then joins and continues to the workspace", async () => {
    vi.mocked(previewSharedInvitation).mockResolvedValue(invitation({ account_exists: true }));
    vi.mocked(loginSharedUser).mockResolvedValue({ access_token: "new-token" } as never);
    vi.mocked(acceptSharedInvitation).mockResolvedValue(accepted as never);
    const props = renderInvite();
    await screen.findByRole("heading", { name: "Sign in to accept" });
    expect(screen.queryByLabelText("Display name")).toBeNull();
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "correct horse battery" } });
    fireEvent.click(screen.getByRole("button", { name: /Sign in and join/ }));
    await waitFor(() => expect(props.onAuthenticated).toHaveBeenCalledWith("new-token", "/workspaces/w1/overview"));
    expect(loginSharedUser).toHaveBeenCalledWith({ email: "ada@example.com", password: "correct horse battery" });
    expect(acceptSharedInvitation).toHaveBeenCalledWith("new-token", "secret");
    expect(registerSharedUser).not.toHaveBeenCalled();
  });

  it("lets someone without an account create it with the invited address and the invitation as proof", async () => {
    vi.mocked(previewSharedInvitation).mockResolvedValue(invitation({ account_exists: false }));
    vi.mocked(registerSharedUser).mockResolvedValue({ access_token: "created" } as never);
    vi.mocked(acceptSharedInvitation).mockResolvedValue(accepted as never);
    const props = renderInvite();
    await screen.findByRole("heading", { name: "Create your account to accept" });
    expect((screen.getByLabelText("Email") as HTMLInputElement).readOnly).toBe(true);
    fireEvent.change(screen.getByLabelText("Display name"), { target: { value: "Ada" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "correct horse battery" } });
    fireEvent.click(screen.getByRole("button", { name: /Create account and join/ }));
    await waitFor(() => expect(props.onAuthenticated).toHaveBeenCalledWith("created", "/workspaces/w1/overview"));
    expect(registerSharedUser).toHaveBeenCalledWith({
      email: "ada@example.com", displayName: "Ada", password: "correct horse battery", invitationToken: "secret",
    });
  });

  it("stays on the invitation when signing in works but joining does not, so it can be retried", async () => {
    vi.mocked(previewSharedInvitation).mockResolvedValue(invitation());
    vi.mocked(loginSharedUser).mockResolvedValue({ access_token: "new-token" } as never);
    vi.mocked(acceptSharedInvitation).mockRejectedValue(new Error("This invitation was withdrawn."));
    const props = renderInvite();
    await screen.findByRole("heading", { name: "Sign in to accept" });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "correct horse battery" } });
    fireEvent.click(screen.getByRole("button", { name: /Sign in and join/ }));
    await waitFor(() => expect(props.onAuthenticated).toHaveBeenCalledWith("new-token", "/invite#secret"));
    expect(showToast).toHaveBeenCalledWith("error", "This invitation was withdrawn.");
  });

  it("offers a signed-in person with the invited address a single button to join", async () => {
    vi.mocked(previewSharedInvitation).mockResolvedValue(invitation());
    vi.mocked(acceptSharedInvitation).mockResolvedValue(accepted as never);
    const props = renderInvite({ accessToken: "mine", session: sessionFor("Ada@Example.com") });
    await screen.findByRole("heading", { name: "Join the Atlas workspace" });
    fireEvent.click(screen.getByRole("button", { name: /Accept the invitation/ }));
    await waitFor(() => expect(props.onJoined).toHaveBeenCalledWith("/workspaces/w1/overview"));
    expect(acceptSharedInvitation).toHaveBeenCalledWith("mine", "secret");
    expect(showToast).toHaveBeenCalledWith("success", "You joined the Atlas workspace.");
  });

  it("tells someone signed in as another account to sign out, and does not offer to join", async () => {
    vi.mocked(previewSharedInvitation).mockResolvedValue(invitation());
    const props = renderInvite({ accessToken: "mine", session: sessionFor("grace@example.com") });
    await screen.findByRole("heading", { name: "This invitation is for ada@example.com" });
    expect(screen.getByText(/You are signed in as grace@example.com/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Accept the invitation/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
    expect(props.onSignOut).toHaveBeenCalled();
    expect(acceptSharedInvitation).not.toHaveBeenCalled();
  });

  it.each([
    ["used", "This invitation was already used."],
    ["expired", "This invitation has expired."],
    ["withdrawn", "This invitation was withdrawn."],
  ])("shows a %s link as unusable, with the server's reason and a way out", async (_state, message) => {
    vi.mocked(previewSharedInvitation).mockRejectedValue(new Error(message));
    renderInvite();
    await screen.findByRole("heading", { name: "This link cannot be used" });
    expect(screen.getByText(message)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Go to sign-in" })).toBeTruthy();
    expect(screen.queryByLabelText("Password")).toBeNull();
  });

  it("points a signed-in person with an unusable link to the dashboard", async () => {
    vi.mocked(previewSharedInvitation).mockRejectedValue(new Error("This invitation has expired."));
    renderInvite({ accessToken: "mine", session: sessionFor("ada@example.com") });
    await screen.findByRole("heading", { name: "This link cannot be used" });
    expect(screen.getByRole("button", { name: "Go to the dashboard" })).toBeTruthy();
  });

  it("falls back to a plain message when the check fails without a reason", async () => {
    vi.mocked(previewSharedInvitation).mockRejectedValue("offline");
    renderInvite();
    await screen.findByText("This invitation link could not be checked.");
  });

  it("goes to the organization for an organization invitation", () => {
    expect(invitationDestination({ ...accepted, target_type: "organization", target_id: "o 1" })).toBe("/workspaces?organization=o%201");
  });
});

describe("password reset page", () => {
  const reset = (overrides: Partial<PasswordResetPreview> = {}): PasswordResetPreview => ({
    email: "ada@example.com", display_name: "Ada", expires_at: "2099-01-01T00:00:00Z", ...overrides,
  });

  it("names the account while checking, then asks for the new password twice", async () => {
    vi.mocked(previewSharedPasswordReset).mockResolvedValue(reset());
    render(<PasswordResetPage onAuthenticated={vi.fn()} token="secret" />);
    expect(screen.getByText("Checking the link…")).toBeTruthy();
    await screen.findByRole("heading", { name: "New password for ada@example.com" });
    expect(screen.getByPlaceholderText("At least 12 characters")).toBeTruthy();
    expect(screen.getByLabelText("Repeat it")).toBeTruthy();
    expect(previewSharedPasswordReset).toHaveBeenCalledWith("secret");
  });

  it("saves the password and signs this browser in", async () => {
    vi.mocked(previewSharedPasswordReset).mockResolvedValue(reset());
    vi.mocked(completeSharedPasswordReset).mockResolvedValue({ access_token: "fresh" } as never);
    const onAuthenticated = vi.fn();
    render(<PasswordResetPage onAuthenticated={onAuthenticated} token="secret" />);
    await screen.findByPlaceholderText("At least 12 characters");
    fireEvent.change(screen.getByPlaceholderText("At least 12 characters"), { target: { value: "a brand new passphrase" } });
    fireEvent.change(screen.getByLabelText("Repeat it"), { target: { value: "a brand new passphrase" } });
    fireEvent.click(screen.getByRole("button", { name: /Save the new password/ }));
    await waitFor(() => expect(onAuthenticated).toHaveBeenCalledWith("fresh"));
    expect(completeSharedPasswordReset).toHaveBeenCalledWith("secret", "a brand new passphrase");
  });

  it("does not send two different passwords", async () => {
    vi.mocked(previewSharedPasswordReset).mockResolvedValue(reset());
    render(<PasswordResetPage onAuthenticated={vi.fn()} token="secret" />);
    await screen.findByPlaceholderText("At least 12 characters");
    fireEvent.change(screen.getByPlaceholderText("At least 12 characters"), { target: { value: "a brand new passphrase" } });
    fireEvent.change(screen.getByLabelText("Repeat it"), { target: { value: "another passphrase entirely" } });
    fireEvent.click(screen.getByRole("button", { name: /Save the new password/ }));
    expect(showToast).toHaveBeenCalledWith("error", "The two passwords are not the same.");
    expect(completeSharedPasswordReset).not.toHaveBeenCalled();
  });

  it("keeps the form and says why when the server refuses the password", async () => {
    vi.mocked(previewSharedPasswordReset).mockResolvedValue(reset());
    vi.mocked(completeSharedPasswordReset).mockRejectedValue(new Error("That password is too common."));
    const onAuthenticated = vi.fn();
    render(<PasswordResetPage onAuthenticated={onAuthenticated} token="secret" />);
    await screen.findByPlaceholderText("At least 12 characters");
    fireEvent.change(screen.getByPlaceholderText("At least 12 characters"), { target: { value: "passwordpassword" } });
    fireEvent.change(screen.getByLabelText("Repeat it"), { target: { value: "passwordpassword" } });
    fireEvent.click(screen.getByRole("button", { name: /Save the new password/ }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "That password is too common."));
    expect(onAuthenticated).not.toHaveBeenCalled();
    expect(screen.getByPlaceholderText("At least 12 characters")).toBeTruthy();
  });

  it.each([
    ["used", "This reset link was already used."],
    ["expired", "This reset link has expired."],
  ])("shows a %s link as unusable with the server's reason", async (_state, message) => {
    vi.mocked(previewSharedPasswordReset).mockRejectedValue(new Error(message));
    render(<PasswordResetPage onAuthenticated={vi.fn()} token="secret" />);
    await screen.findByRole("heading", { name: "This link cannot be used" });
    expect(screen.getByText(message)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Go to sign-in" })).toBeTruthy();
    expect(screen.queryByPlaceholderText("At least 12 characters")).toBeNull();
  });

  it("falls back to a plain message when the check fails without a reason", async () => {
    vi.mocked(previewSharedPasswordReset).mockRejectedValue("offline");
    render(<PasswordResetPage onAuthenticated={vi.fn()} token="secret" />);
    await screen.findByText("This link could not be checked.");
  });
});
