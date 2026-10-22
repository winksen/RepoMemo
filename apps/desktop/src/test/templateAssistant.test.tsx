import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
import type { SharedWorkspace, WorkspaceTemplateView } from "../types";
import { paletteEntries } from "../web/paletteEntries";
import { orderCapabilities, retrievalChips, suggestedQuestions } from "../web/templateAssistant";
import { useTemplate } from "../web/workspace/data";

beforeEach(() => { vi.clearAllMocks(); queryClient.clear(); });
afterEach(cleanup);

const offered = [{ id: "find_files" }, { id: "find_symbol" }, { id: "search_content" }, { id: "summarize_file" }, { id: "ask_question" }, { id: "workspace_overview" }];
const ids = (list: Array<{ id: string }>) => list.map((item) => item.id);

describe("the assistant's buttons", () => {
  it("keep the server's order without a template", () => {
    expect(ids(orderCapabilities(offered, null))).toEqual(ids(offered));
    expect(ids(orderCapabilities(offered, { order: [] }))).toEqual(ids(offered));
  });

  it("list the template's preferred jobs first, in its order, and the rest after them in the server's order", () => {
    expect(ids(orderCapabilities(offered, { order: ["ask_question", "search_content", "find_files"] }))).toEqual(["ask_question", "search_content", "find_files", "find_symbol", "summarize_file", "workspace_overview"]);
  });

  it("ignore a job the server did not offer and a job named twice", () => {
    const ordered = ids(orderCapabilities(offered, { order: ["book_flights", "ask_question", "ask_question"] }));
    expect(ordered[0]).toBe("ask_question");
    expect(ordered).toHaveLength(offered.length);
  });
});

describe("the suggested questions", () => {
  it("are what the template gave, trimmed, without blanks or repeats", () => {
    expect(suggestedQuestions(null)).toEqual([]);
    expect(suggestedQuestions({ suggestions: ["What is the runbook for…", "  Who is on call?  ", "", "Who is on call?"] })).toEqual(["What is the runbook for…", "Who is on call?"]);
  });
});

function templateWith(assistant: WorkspaceTemplateView["assistant"]): WorkspaceTemplateView {
  return {
    template: { id: "operations", version: 1, name: "Operations and incidents", summary: "", icon: "heartbeat" },
    source: "workspace", reason: null, chosen_id: "operations", overrides: null,
    sections: { primary: [], more: [], landing: "overview" }, labels: { sections: {}, artifact_types: {} }, overview: { cards: [] },
    evidence: { default_view: "grid", types_first: [] }, assistant, retrieval: { type_chips: [] }, ranking: { verified: 1.1, boosts: [], recency: null }, health: { detectors: ["index_failed"], stale_after_days: null }, review: { order: [] },
  };
}

function renderPanel() {
  vi.mocked(getSharedAgentCapabilities).mockResolvedValue({
    provider_name: null,
    capabilities: ["find_files", "search_content", "ask_question"].map((id) => ({ id, label: id.replace("_", " "), description: "", placeholder: "", requires_ai: false, available: true })),
  } as never);
  vi.mocked(listSharedAgentConversations).mockResolvedValue([]);
  return render(<QueryClientProvider client={queryClient}><AssistantPanel accessToken="t" onOpenArtifact={vi.fn()} workspaceId="w1" /></QueryClientProvider>);
}

describe("the assistant panel with a template", () => {
  it("lists its jobs in the template's order and suggests its questions, which fill the box and send nothing", async () => {
    vi.mocked(useTemplate).mockReturnValue([templateWith({ order: ["ask_question", "find_files"], suggestions: ["What is the runbook for…", "Who is on call?"] }), vi.fn(), {} as never]);
    renderPanel();
    const actions = await screen.findByRole("group", { name: "Assistant actions" });
    await waitFor(() => expect(within(actions).getAllByRole("button").map((button) => button.textContent)).toEqual(["ask question", "find files", "search content"]));
    const suggestions = screen.getByRole("group", { name: "Suggested questions" });
    expect(within(suggestions).getAllByRole("button").map((button) => button.textContent)).toEqual(["What is the runbook for…", "Who is on call?"]);
    fireEvent.click(within(suggestions).getByRole("button", { name: "What is the runbook for…" }));
    expect((screen.getByRole("textbox", { name: "Message the assistant" }) as HTMLTextAreaElement).value).toBe("What is the runbook for…");
    expect(sendSharedAgentMessage).not.toHaveBeenCalled();
    // Once there is something in the box the suggestions step aside.
    expect(screen.queryByRole("group", { name: "Suggested questions" })).toBeNull();
  });

  it("keeps the usual order and suggests nothing for a workspace with no template", async () => {
    vi.mocked(useTemplate).mockReturnValue([null, vi.fn(), {} as never]);
    renderPanel();
    const actions = await screen.findByRole("group", { name: "Assistant actions" });
    await waitFor(() => expect(within(actions).getAllByRole("button").map((button) => button.textContent)).toEqual(["find files", "search content", "ask question"]));
    expect(screen.queryByRole("group", { name: "Suggested questions" })).toBeNull();
  });
});

describe("the Retrieval chips", () => {
  it("are the template's kinds of evidence, once each, and only those the workspace has files of when that is known", () => {
    expect(retrievalChips(["runbook", "incident", "runbook"], null)).toEqual(["runbook", "incident"]);
    expect(retrievalChips(["runbook", "incident", "note"], ["note", "runbook", "image"])).toEqual(["runbook", "note"]);
    expect(retrievalChips(undefined, ["note"])).toEqual([]);
    expect(retrievalChips(["note"], [])).toEqual([]);
  });
});

describe("the command palette's assistant commands", () => {
  const workspace: SharedWorkspace = { workspace: { id: "w1", name: "Ops", created_at: "", updated_at: "", settings: {} }, organization_id: "o1", role: "owner" };
  const input = (assistantOrder?: string[]) => ({ query: "", workspaces: [workspace], assistantWorkspace: workspace, currentWorkspace: workspace, files: [], cards: [], canAdministerSystem: false, assistantOrder });
  const commands = (entries: ReturnType<typeof paletteEntries>) => entries.filter((entry) => entry.group === "Assistant").map((entry) => entry.id);

  it("are in the usual order without a template", () => {
    expect(commands(paletteEntries(input()))).toEqual(["assistant:ask_question", "assistant:find_files", "assistant:search_content", "assistant:find_symbol"]);
  });

  it("follow the template's preferred order", () => {
    expect(commands(paletteEntries(input(["find_symbol", "search_content"])))).toEqual(["assistant:find_symbol", "assistant:search_content", "assistant:ask_question", "assistant:find_files"]);
  });
});
