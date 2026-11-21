import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", async (original) => ({ ...(await original<typeof import("../lib/sharedApi")>()), createSharedWorkspace: vi.fn(), saveSharedWorkspaceColor: vi.fn(), saveSharedWorkspaceIcon: vi.fn() }));
vi.mock("../web/workspace/data", () => ({ useTemplateCatalogue: vi.fn(() => [[], vi.fn(), {}]) }));

import { NewWorkspaceDialog, creatableOrganizations } from "../components/NewWorkspaceDialog";
import { onNewWorkspaceRequest, openNewWorkspace } from "../lib/newWorkspaceBridge";
import { createSharedWorkspace, saveSharedWorkspaceColor, saveSharedWorkspaceIcon } from "../lib/sharedApi";
import type { Organization, SharedWorkspace } from "../types";
import { WORKSPACE_ICONS } from "../web/workspaceIcons";

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

it("says how the workspace will be named, and keeps it up to date as the organization and the name change", () => {
  render(<NewWorkspaceDialog accessToken="t" defaultOrganizationId="o1" onClose={vi.fn()} onCreated={vi.fn()} open organizations={organizations} />);
  expect(screen.getByText(/Workspace will be created as/).textContent).toContain("Bend / Workspace name");
  fireEvent.change(screen.getByLabelText(/Workspace name/), { target: { value: "Payments" } });
  expect(screen.getByText(/Workspace will be created as/).textContent).toContain("Bend / Payments");
});

it("saves the icon and the color that were chosen once the workspace exists, and hands them to the list", async () => {
  const created = { organization_id: "o1", workspace: { id: "w9", name: "Payments", settings: {} } } as unknown as SharedWorkspace;
  vi.mocked(createSharedWorkspace).mockResolvedValue(created);
  vi.mocked(saveSharedWorkspaceIcon).mockResolvedValue({ icon: WORKSPACE_ICONS[1].key });
  vi.mocked(saveSharedWorkspaceColor).mockResolvedValue({ color: "teal" });
  const onCreated = vi.fn();
  render(<NewWorkspaceDialog accessToken="t" defaultOrganizationId="o1" onClose={vi.fn()} onCreated={onCreated} open organizations={organizations} />);
  fireEvent.change(screen.getByLabelText(/Workspace name/), { target: { value: "Payments" } });
  fireEvent.click(screen.getByRole("button", { name: "Teal" }));
  fireEvent.click(screen.getByRole("button", { name: WORKSPACE_ICONS[1].label }));
  fireEvent.click(screen.getByRole("button", { name: "Create workspace" }));
  await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
  expect(saveSharedWorkspaceIcon).toHaveBeenCalledWith("t", "w9", WORKSPACE_ICONS[1].key);
  expect(saveSharedWorkspaceColor).toHaveBeenCalledWith("t", "w9", "teal");
  expect(onCreated.mock.calls[0][0].workspace.settings.color).toBe("teal");
});

it("explains inherited and restricted access in words", () => {
  render(<NewWorkspaceDialog accessToken="t" defaultOrganizationId="o1" onClose={vi.fn()} onCreated={vi.fn()} open organizations={organizations} />);
  expect(screen.getByText("Access inherited from the organization")).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "Restrict access to the people you add" }));
  expect(screen.getByText("Restricted access")).toBeTruthy();
  expect(screen.getByText(/Only the people you add one by one/)).toBeTruthy();
});
