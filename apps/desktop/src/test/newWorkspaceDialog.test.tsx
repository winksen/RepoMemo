import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", async (original) => ({ ...(await original<typeof import("../lib/sharedApi")>()), createSharedWorkspace: vi.fn() }));
vi.mock("../web/workspace/data", () => ({ useTemplateCatalogue: vi.fn(() => [[], vi.fn(), {}]) }));

import { NewWorkspaceDialog, creatableOrganizations } from "../components/NewWorkspaceDialog";
import { onNewWorkspaceRequest, openNewWorkspace } from "../lib/newWorkspaceBridge";
import { createSharedWorkspace } from "../lib/sharedApi";
import type { Organization, SharedWorkspace } from "../types";

const organization = (id: string, name: string, role: Organization["role"]) => ({ id, name, role }) as Organization;
const organizations = [organization("o1", "Bend", "owner"), organization("o2", "Other", "member"), organization("o3", "ANA", "admin")];

beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);

it("offers only the organizations where the person can create workspaces", () => {
  expect(creatableOrganizations(organizations).map((entry) => entry.id)).toEqual(["o1", "o3"]);
});

it("opens with the organization it was opened from chosen, and creates the workspace in it", async () => {
  const created = { organization_id: "o3", workspace: { id: "w9", name: "Payments" } } as SharedWorkspace;
  vi.mocked(createSharedWorkspace).mockResolvedValue(created);
  const onCreated = vi.fn();
  const onClose = vi.fn();
  render(<NewWorkspaceDialog accessToken="t" defaultOrganizationId="o3" onClose={onClose} onCreated={onCreated} open organizations={organizations} />);
  expect(screen.getByRole("combobox", { name: /Organization/ }).textContent).toContain("ANA");
  fireEvent.change(screen.getByLabelText(/Workspace name/), { target: { value: "Payments" } });
  fireEvent.click(screen.getByRole("button", { name: "Create workspace" }));
  await waitFor(() => expect(onCreated).toHaveBeenCalledWith(created));
  expect(createSharedWorkspace).toHaveBeenCalledWith("t", "o3", "Payments", undefined, false);
  expect(onClose).toHaveBeenCalled();
});

it("says who can create workspaces when the person administers no organization", () => {
  render(<NewWorkspaceDialog accessToken="t" onClose={vi.fn()} onCreated={vi.fn()} open organizations={[organization("o2", "Other", "member")]} />);
  expect(screen.getByText(/Only an organization's owners and administrators/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Create workspace" })).toBeNull();
});

it("passes the organization from the + menu or an organization page to whoever listens", () => {
  const listener = vi.fn();
  const stop = onNewWorkspaceRequest(listener);
  openNewWorkspace("o1");
  openNewWorkspace();
  stop();
  openNewWorkspace("o2");
  expect(listener.mock.calls).toEqual([["o1"], [undefined]]);
});
