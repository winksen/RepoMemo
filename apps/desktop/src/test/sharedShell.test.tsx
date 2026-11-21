import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

vi.mock("../web/router", async (original) => ({ ...(await original<typeof import("../web/router")>()), navigate: vi.fn() }));

import { SharedLayout, SharedShell } from "../components/SharedLayout";
import type { SharedSession } from "../types";

afterEach(cleanup);

const session = { user: { id: "u1", email: "a@example.com", display_name: "Ada" }, authentication: "jwt", memberships: [] } as unknown as SharedSession;
const props = { onNavigate: vi.fn(), session, sidebar: <nav aria-label="Rail list">rail</nav>, signOut: vi.fn() };
const page = (text: string) => <SharedLayout {...props} workspaceNavigation={<div>{`tabs of ${text}`}</div>}><p>{text}</p></SharedLayout>;

it("keeps the top bar and the rail in place while the page inside the shell changes", () => {
  const { rerender } = render(<SharedShell {...props} scrollKey="a">{page("first page")}</SharedShell>);
  const header = document.querySelector(".shared-home-header");
  const rail = document.querySelector(".shared-home-rail");
  expect(screen.getByText("first page")).toBeTruthy();
  expect(screen.getByText("tabs of first page")).toBeTruthy();

  rerender(<SharedShell {...props} scrollKey="b">{<SharedLayout {...props}><h2>second page</h2></SharedLayout>}</SharedShell>);
  expect(screen.getByText("second page")).toBeTruthy();
  expect(screen.queryByText("first page")).toBeNull();
  expect(screen.queryByText("tabs of first page")).toBeNull();
  expect(document.querySelector(".shared-home-header")).toBe(header);
  expect(document.querySelector(".shared-home-rail")).toBe(rail);
  expect(document.querySelectorAll(".shared-home")).toHaveLength(1);
});

it("is a whole frame by itself, outside the shell", () => {
  render(page("alone"));
  expect(screen.getByText("alone")).toBeTruthy();
  expect(document.querySelector(".shared-home-rail")).toBeTruthy();
});
