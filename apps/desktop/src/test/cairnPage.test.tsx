import { cleanup, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../components/AssistantPanel", () => ({ AssistantPanel: ({ workspaceId, requestedChatId, composerHeader }: { workspaceId: string; requestedChatId?: string | null; composerHeader?: React.ReactNode }) => <div><div data-testid="panel">cairn for {workspaceId} chat {String(requestedChatId)}</div>{composerHeader}</div> }));
vi.mock("../lib/sharedApi", async (original) => ({ ...(await original<typeof import("../lib/sharedApi")>()), listSharedAgentConversations: vi.fn(), getSharedAgentCapabilities: vi.fn() }));
vi.mock("../web/router", async (original) => ({ ...(await original<typeof import("../web/router")>()), navigate: vi.fn() }));

import { CairnRail, SharedCairnPage } from "../web/CairnPage";
import { cairnContextWorkspace } from "../web/cairn";
import { getSharedAgentCapabilities, listSharedAgentConversations } from "../lib/sharedApi";
import { navigate } from "../web/router";
import type { AgentConversation, Organization, SharedSession, SharedWorkspace } from "../types";

const entry = (id: string, name: string) => ({ organization_id: "o1", role: "owner", workspace: { id, name, settings: {} } }) as unknown as SharedWorkspace;
const workspaces = [entry("w1", "Code"), entry("w2", "Pay")];
const organizations = [{ id: "o1", name: "Bend", role: "owner" }] as Organization[];
const session = { user: { id: "u1", email: "a@example.com", display_name: "Ada" }, authentication: "jwt", memberships: [] } as unknown as SharedSession;
const chat = (id: string, workspace_id: string, title: string, updated_at: string) => ({ id, workspace_id, title, turn_count: 2, created_at: updated_at, updated_at }) as AgentConversation;
const wrap = (node: React.ReactNode) => <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{node}</QueryClientProvider>;

beforeEach(() => {
  vi.clearAllMocks();
  window.localStorage.clear();
  vi.mocked(getSharedAgentCapabilities).mockResolvedValue({ capabilities: [], provider_name: "Local model", model: "llama-3" } as never);
  vi.mocked(listSharedAgentConversations).mockImplementation(async (_token, workspaceId) => workspaceId === "w1"
    ? [chat("c1", "w1", "Retry policy", "2026-11-20T10:00:00Z")]
    : [chat("c2", "w2", "Refund rules", "2026-11-21T10:00:00Z")]);
});
afterEach(cleanup);

it("starts a new chat in the workspace last worked in, else the first", () => {
  expect(cairnContextWorkspace(workspaces, "w2")?.workspace.id).toBe("w2");
  expect(cairnContextWorkspace(workspaces, "gone")?.workspace.id).toBe("w1");
  expect(cairnContextWorkspace([], null)).toBeUndefined();
});

it("groups the chats in the sidebar by workspace, the workspace with the latest chat first, and opens one", async () => {
  render(wrap(<CairnRail accessToken="t" chatId="c1" workspaces={workspaces} />));
  const groups = await screen.findAllByRole("region");
  expect(groups.map((group) => group.getAttribute("aria-label"))).toEqual(["Pay", "Code"]);
  expect(screen.getByRole("button", { name: "Retry policy" }).getAttribute("aria-current")).toBe("page");
  screen.getByRole("button", { name: "Refund rules" }).click();
  expect(navigate).toHaveBeenCalledWith("/cairn/c2");
  screen.getByRole("button", { name: "New chat in Code" }).click();
  expect(window.localStorage.getItem("repomemo.cairn.workspace")).toBe("w1");
  expect(navigate).toHaveBeenLastCalledWith("/cairn");
});

it("shows Cairn for the workspace of the open chat, as a label and not a choice", async () => {
  render(wrap(<SharedCairnPage accessToken="t" chatId="c2" organizations={organizations} session={session} signOut={vi.fn()} workspaces={workspaces} />));
  expect((await screen.findByTestId("panel")).textContent).toBe("cairn for w2 chat c2");
  // The heading is the chat's title, and the model in use is under it.
  expect(await screen.findByRole("heading", { name: "Refund rules" })).toBeTruthy();
  expect(await screen.findByText("Model: llama-3 · Local model")).toBeTruthy();
  expect(screen.queryByRole("combobox")).toBeNull();
  expect(screen.getByTitle("The workspace this chat draws on").textContent).toContain("Pay");
});

it("offers a choice of workspace for a new chat, starting from the remembered one", async () => {
  window.localStorage.setItem("repomemo.cairn.workspace", "w2");
  const { unmount } = render(wrap(<SharedCairnPage accessToken="t" chatId={null} organizations={organizations} session={session} signOut={vi.fn()} workspaces={workspaces} />));
  expect((await screen.findByTestId("panel")).textContent).toBe("cairn for w2 chat null");
  expect(screen.getByRole("button", { name: "Workspace for this chat" }).textContent).toContain("Pay");
  unmount();
  render(wrap(<SharedCairnPage accessToken="t" chatId={null} organizations={organizations} session={session} signOut={vi.fn()} workspaces={[]} />));
  expect(screen.getByText("No workspace to talk about yet")).toBeTruthy();
});

it("shows the workspace above the box as a plain label once the chat has begun", async () => {
  render(wrap(<SharedCairnPage accessToken="t" chatId="c1" organizations={organizations} session={session} signOut={vi.fn()} workspaces={workspaces} />));
  await screen.findByTestId("panel");
  expect(screen.queryByRole("button", { name: "Workspace for this chat" })).toBeNull();
  expect(screen.getByTitle("The workspace this chat draws on").textContent).toContain("Code");
});
