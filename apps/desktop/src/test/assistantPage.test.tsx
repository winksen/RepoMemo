import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../components/AssistantPanel", () => ({ AssistantPanel: ({ workspaceId }: { workspaceId: string }) => <div data-testid="panel">assistant for {workspaceId}</div> }));
vi.mock("../web/router", async (original) => ({ ...(await original<typeof import("../web/router")>()), navigate: vi.fn() }));

import { SharedAssistantPage, assistantWorkspace } from "../web/AssistantPage";
import { navigate } from "../web/router";
import type { Organization, SharedSession, SharedWorkspace } from "../types";

const entry = (id: string, name: string) => ({ organization_id: "o1", role: "owner", workspace: { id, name, settings: {} } }) as unknown as SharedWorkspace;
const workspaces = [entry("w1", "Code"), entry("w2", "Pay")];
const organizations = [{ id: "o1", name: "Bend", role: "owner" }] as Organization[];
const session = { user: { id: "u1", email: "a@example.com", display_name: "Ada" }, authentication: "jwt", memberships: [] } as unknown as SharedSession;

beforeEach(() => { vi.clearAllMocks(); window.localStorage.clear(); });
afterEach(cleanup);

it("chooses the workspace named by the address, else the last one used, else the first", () => {
  expect(assistantWorkspace(workspaces, "w2", "w1")?.workspace.id).toBe("w2");
  expect(assistantWorkspace(workspaces, "gone", "w2")?.workspace.id).toBe("w2");
  expect(assistantWorkspace(workspaces, null, "gone")?.workspace.id).toBe("w1");
  expect(assistantWorkspace([], null, null)).toBeUndefined();
});

it("shows the assistant for the workspace of the address, and remembers it", () => {
  render(<SharedAssistantPage accessToken="t" organizations={organizations} requestedWorkspaceId="w2" session={session} signOut={vi.fn()} workspaces={workspaces} />);
  expect(screen.getByRole("heading", { name: "Cairn" })).toBeTruthy();
  expect(screen.getByTestId("panel").textContent).toBe("assistant for w2");
  expect(window.localStorage.getItem("repomemo.assistant.page-workspace")).toBe("w2");
  expect(navigate).not.toHaveBeenCalled();
});

it("puts the chosen workspace in the address when it was not named", () => {
  render(<SharedAssistantPage accessToken="t" organizations={organizations} requestedWorkspaceId={null} session={session} signOut={vi.fn()} workspaces={workspaces} />);
  expect(screen.getByTestId("panel").textContent).toBe("assistant for w1");
  expect(navigate).toHaveBeenCalledWith("/assistant/w1", true);
});

it("says there is nothing to talk about when the person has no workspace", () => {
  render(<SharedAssistantPage accessToken="t" organizations={organizations} requestedWorkspaceId={null} session={session} signOut={vi.fn()} workspaces={[]} />);
  expect(screen.queryByTestId("panel")).toBeNull();
  expect(screen.getByText("No workspace to talk about yet")).toBeTruthy();
  expect(screen.getByRole("button", { name: "New workspace" })).toBeTruthy();
});
