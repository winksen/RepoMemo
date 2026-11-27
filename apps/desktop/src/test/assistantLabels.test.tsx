import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  deleteSharedAgentConversation: vi.fn(),
  getSharedAgentCapabilities: vi.fn(),
  getSharedAgentConversation: vi.fn(),
  listSharedAgentConversations: vi.fn(),
  renameSharedAgentConversation: vi.fn(),
  sendSharedAgentMessage: vi.fn(),
}));
vi.mock("../web/workspace/data", () => ({ useTemplate: vi.fn() }));
vi.mock("../web/common", () => ({ ViewAtCommit: () => null }));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { AssistantPanel } from "../components/AssistantPanel";
import { queryClient } from "../lib/queryClient";
import { getSharedAgentCapabilities, listSharedAgentConversations, sendSharedAgentMessage } from "../lib/sharedApi";
import { useTemplate } from "../web/workspace/data";

beforeEach(() => { vi.clearAllMocks(); queryClient.clear(); vi.mocked(useTemplate).mockReturnValue([null, vi.fn(), {} as never]); });
afterEach(cleanup);

const reply = (extra: Record<string, unknown>) => ({
  capability: "find_files", routing: "rules", subject: "", reply_markdown: "Found 1 file.", generated: false,
  files: [], matches: [], citations: [], warnings: [], ...extra,
});

async function ask(replyFields: Record<string, unknown>, providerName: string | null = null) {
  vi.mocked(getSharedAgentCapabilities).mockResolvedValue({
    provider_name: providerName,
    capabilities: [{ id: "find_files", label: "find files", description: "", placeholder: "", requires_ai: false, available: true }],
  } as never);
  vi.mocked(listSharedAgentConversations).mockResolvedValue([]);
  vi.mocked(sendSharedAgentMessage).mockResolvedValue({
    conversation: { id: "c1", title: "Chat", updated_at: "2026-11-27T10:00:00Z" },
    turn: { id: "t1", conversation_id: "c1", position: 1, label: "find the readme", request: { message: "find the readme", capability: null }, reply: reply(replyFields), created_at: "2026-11-27T10:00:00Z" },
  } as never);
  render(<QueryClientProvider client={queryClient}><AssistantPanel accessToken="t" onOpenArtifact={vi.fn()} workspaceId="w1" /></QueryClientProvider>);
  const box = await screen.findByRole("textbox", { name: "Message CAIRN" });
  fireEvent.change(box, { target: { value: "find the readme" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await screen.findByText("Found 1 file.");
}

describe("the labels under CAIRN's replies (V0.9.142)", () => {
  it("keeps one standing line under the message box and says nothing about how a reply was routed", async () => {
    await ask({ routing: "rules", generated: false, ai_unavailable_reason: null }, "Local model");
    expect(screen.getAllByText("AI responses can be inaccurate. Check the cited sources.").length).toBe(1);
    expect(screen.queryByText(/matched from your wording/i)).toBeNull();
    expect(screen.queryByText(/interpreted by/i)).toBeNull();
    expect(screen.queryByText(/worked out with its tools/i)).toBeNull();
    expect(screen.queryByText(/AI-generated/i)).toBeNull();
    expect(screen.queryByText(/Answered without AI/i)).toBeNull();
  });

  it("shows no AI-generated chip on a reply a model wrote", async () => {
    await ask({ routing: "agent", generated: true, capability: null }, "Local model");
    expect(screen.queryByText(/AI-generated/i)).toBeNull();
    expect(screen.queryByText(/worked out with its tools/i)).toBeNull();
    expect(screen.queryByText(/Answered without AI/i)).toBeNull();
  });

  it("labels a reply made without AI with the reason the server gave", async () => {
    await ask({ routing: "rules", generated: false, ai_unavailable_reason: "Your workspace keeps AI to owners and admins." });
    expect(await screen.findByText("Answered without AI · Your workspace keeps AI to owners and admins.")).toBeTruthy();
    await waitFor(() => expect(screen.queryByText(/matched from your wording/i)).toBeNull());
  });
});
