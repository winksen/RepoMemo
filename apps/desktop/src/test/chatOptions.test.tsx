import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";
import { ChatOptions, chatFacts } from "../components/ChatOptions";
import type { AgentCapabilities, AgentConversation, AgentReply } from "../types";

beforeAll(() => {
  Object.assign(window.HTMLElement.prototype, { hasPointerCapture: () => false, releasePointerCapture: () => undefined, scrollIntoView: () => undefined, setPointerCapture: () => undefined });
  window.ResizeObserver = window.ResizeObserver ?? class { disconnect() { /* none */ } observe() { /* none */ } unobserve() { /* none */ } };
});
afterEach(cleanup);

const citation = (id: string, title: string) => ({ artifact_id: id, chunk_id: null, title, path: `docs/${title}`, start_line: null, end_line: null, confidence: null, commit: null, changed_by: null }) as unknown as AgentReply["citations"][number];
const reply = (capability: AgentReply["capability"], generated: boolean, citations: AgentReply["citations"], files: AgentReply["files"] = []): AgentReply => ({ capability, routing: "rules", subject: "", reply_markdown: "", generated, files, matches: [], citations, warnings: [] });
const capabilities = { provider_name: "Local model", capabilities: [{ id: "search_content", label: "Search content", description: "", available: true, placeholder: "" }] } as unknown as AgentCapabilities;
const conversation = { id: "c1", workspace_id: "w1", title: "Retry policy", turn_count: 2, created_at: "2026-11-20T10:00:00Z", updated_at: "2026-11-21T10:00:00Z" } as AgentConversation;

it("counts what a chat has used from its replies: exchanges, distinct files, actions and AI-written replies", () => {
  const facts = chatFacts([
    reply("search_content", true, [citation("a1", "retry.md"), citation("a2", "queue.md")]),
    reply("search_content", false, [citation("a1", "retry.md")], [{ id: "a3", title: "billing.md" } as never]),
    undefined,
  ]);
  expect(facts.exchanges).toBe(3);
  expect(facts.files.map((file) => file.id)).toEqual(["a1", "a2", "a3"]);
  expect(facts.capabilities).toEqual([{ id: "search_content", count: 2 }]);
  expect([facts.generated, facts.replies]).toEqual([1, 2]);
});

it("shows the chat's facts and files in a dropdown, opens a file, and renames or deletes the chat", () => {
  const onOpenArtifact = vi.fn();
  const onRename = vi.fn();
  const onDelete = vi.fn();
  render(<ChatOptions capabilities={capabilities} conversation={conversation} onDelete={onDelete} onOpenArtifact={onOpenArtifact} onRename={onRename} replies={[reply("search_content", true, [citation("a1", "retry.md")])]} />);
  fireEvent.click(screen.getByRole("button", { name: "Chat options" }));
  expect(screen.getByText(/2 messages back and forth/)).toBeTruthy();
  expect(screen.getByText("Search content")).toBeTruthy();
  expect(screen.getByText(/Files used \(1\)/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "retry.md" }));
  expect(onOpenArtifact).toHaveBeenCalledWith("a1");
  fireEvent.click(screen.getByRole("button", { name: "Chat options" }));
  fireEvent.click(screen.getByRole("button", { name: /Rename/ }));
  expect(onRename).toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Chat options" }));
  fireEvent.click(screen.getByRole("button", { name: /Delete/ }));
  expect(onDelete).toHaveBeenCalled();
});

it("says no file has been used in a new chat, and offers no rename or delete before it exists", () => {
  render(<ChatOptions capabilities={capabilities} conversation={null} onDelete={vi.fn()} onOpenArtifact={vi.fn()} onRename={vi.fn()} replies={[]} />);
  fireEvent.click(screen.getByRole("button", { name: "Chat options" }));
  expect(screen.getByText("No file has been used yet in this chat.")).toBeTruthy();
  expect(screen.queryByRole("button", { name: /Rename/ })).toBeNull();
});
