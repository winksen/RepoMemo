import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", async (original) => ({ ...(await original<typeof import("../lib/sharedApi")>()), createSharedOrganization: vi.fn(), createSharedWorkspace: vi.fn(), saveSharedWorkspaceColor: vi.fn(), saveSharedWorkspaceIcon: vi.fn() }));
vi.mock("../web/workspace/data", () => ({ useTemplateCatalogue: vi.fn(() => [[], vi.fn(), {}]) }));

import { NewOrganizationDialog } from "../components/NewOrganizationDialog";
import { openNewOrganization, onNewOrganizationRequest } from "../lib/newWorkspaceBridge";
import { createSharedOrganization, createSharedWorkspace, saveSharedWorkspaceColor } from "../lib/sharedApi";
import type { Organization, SharedWorkspace } from "../types";

const organization = { id: "o9", name: "Engineering", role: "owner" } as Organization;
const workspace = { organization_id: "o9", workspace: { id: "w9", name: "General", settings: {} } } as unknown as SharedWorkspace;

beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);

it("says how the organization and its first workspace will be named", () => {
  render(<NewOrganizationDialog accessToken="t" onClose={vi.fn()} onCreated={vi.fn()} open />);
  expect(screen.getByText(/Organization will be created as/).textContent).toContain("with the workspace Organization name / General");
  fireEvent.change(screen.getByLabelText(/Organization name/), { target: { value: "Engineering" } });
  fireEvent.change(screen.getByLabelText(/First workspace name/), { target: { value: "Platform" } });
  expect(screen.getByText(/Organization will be created as/).textContent).toContain("Engineering / Platform");
});

it("creates the organization and its first workspace, with the color that was chosen, and hands both on", async () => {
  vi.mocked(createSharedOrganization).mockResolvedValue(organization);
  vi.mocked(createSharedWorkspace).mockResolvedValue(workspace);
  vi.mocked(saveSharedWorkspaceColor).mockResolvedValue({ color: "teal" });
  const onCreated = vi.fn();
  render(<NewOrganizationDialog accessToken="t" onClose={vi.fn()} onCreated={onCreated} open />);
  fireEvent.change(screen.getByLabelText(/Organization name/), { target: { value: "Engineering" } });
  fireEvent.click(screen.getByRole("button", { name: "Teal" }));
  fireEvent.click(screen.getByRole("button", { name: "Create organization" }));
  await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
  expect(createSharedOrganization).toHaveBeenCalledWith("t", "Engineering");
  expect(createSharedWorkspace).toHaveBeenCalledWith("t", "o9", "General", undefined, false);
  expect(onCreated.mock.calls[0][0]).toBe(organization);
  expect(onCreated.mock.calls[0][1].workspace.settings.color).toBe("teal");
});

it("can start empty: no workspace is made, and its fields are not asked for", async () => {
  vi.mocked(createSharedOrganization).mockResolvedValue(organization);
  const onCreated = vi.fn();
  render(<NewOrganizationDialog accessToken="t" onClose={vi.fn()} onCreated={onCreated} open />);
  fireEvent.click(screen.getByRole("switch", { name: "Create a first workspace" }));
  expect(screen.getByText("Start empty")).toBeTruthy();
  expect(screen.queryByRole("group", { name: "Workspace icon" })).toBeNull();
  fireEvent.change(screen.getByLabelText(/Organization name/), { target: { value: "Engineering" } });
  fireEvent.click(screen.getByRole("button", { name: "Create organization" }));
  await waitFor(() => expect(onCreated).toHaveBeenCalledWith(organization, null));
  expect(createSharedWorkspace).not.toHaveBeenCalled();
});

it("keeps the organization when its first workspace cannot be made", async () => {
  vi.mocked(createSharedOrganization).mockResolvedValue(organization);
  vi.mocked(createSharedWorkspace).mockRejectedValue(new Error("refused"));
  const onCreated = vi.fn();
  render(<NewOrganizationDialog accessToken="t" onClose={vi.fn()} onCreated={onCreated} open />);
  fireEvent.change(screen.getByLabelText(/Organization name/), { target: { value: "Engineering" } });
  fireEvent.click(screen.getByRole("button", { name: "Create organization" }));
  await waitFor(() => expect(onCreated).toHaveBeenCalledWith(organization, null));
});

it("passes the request to open the dialog to whoever listens", () => {
  const listener = vi.fn();
  const stop = onNewOrganizationRequest(listener);
  openNewOrganization();
  stop();
  openNewOrganization();
  expect(listener).toHaveBeenCalledTimes(1);
});
