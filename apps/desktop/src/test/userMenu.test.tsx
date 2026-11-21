import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";

vi.mock("../components/UserAvatar", () => ({ UserAvatar: ({ name }: { name: string }) => <span data-testid="avatar">{name}</span> }));

import { UserMenu } from "../components/UserMenu";
import type { SharedSession } from "../types";

// Radix measures and scrolls; jsdom has neither.
beforeAll(() => {
  Object.assign(window.HTMLElement.prototype, { hasPointerCapture: () => false, releasePointerCapture: () => undefined, scrollIntoView: () => undefined, setPointerCapture: () => undefined });
  window.ResizeObserver = window.ResizeObserver ?? class { disconnect() { /* none */ } observe() { /* none */ } unobserve() { /* none */ } };
});
afterEach(cleanup);

const session = { user: { id: "u1", email: "ada@example.com", display_name: "Ada" }, authentication: "jwt", memberships: [] } as unknown as SharedSession;
const open = () => fireEvent.keyDown(screen.getByRole("button", { name: "Account menu for Ada" }), { key: "ArrowDown" });

it("shows who is signed in at the foot of the sidebar", () => {
  render(<UserMenu onNavigate={vi.fn()} session={session} signOut={vi.fn()} />);
  expect(screen.getByRole("button", { name: "Account menu for Ada" }).textContent).toContain("ada@example.com");
});

it("opens a menu with the profile, the settings and log out", async () => {
  const onNavigate = vi.fn();
  const signOut = vi.fn();
  render(<UserMenu onNavigate={onNavigate} session={session} signOut={signOut} />);
  open();
  fireEvent.click(await screen.findByRole("menuitem", { name: "Profile" }));
  expect(onNavigate).toHaveBeenCalledWith("/profile");
  open();
  fireEvent.click(await screen.findByRole("menuitem", { name: "Settings" }));
  expect(onNavigate).toHaveBeenCalledWith("/profile#settings");
  open();
  fireEvent.click(await screen.findByRole("menuitem", { name: "Log out" }));
  expect(signOut).toHaveBeenCalledTimes(1);
});
