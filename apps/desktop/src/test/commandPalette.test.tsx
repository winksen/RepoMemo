import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ querySharedArtifacts: vi.fn(), searchSharedMemoryCards: vi.fn() }));
vi.mock("../web/workspace/data", () => ({ useTemplate: () => [null] }));
vi.mock("../web/router", async (original) => ({ ...(await original<typeof import("../web/router")>()), navigate: vi.fn() }));

import { querySharedArtifacts, searchSharedMemoryCards } from "../lib/sharedApi";
import { takeAssistantCommand, takeNewNoteRequest } from "../lib/assistantBridge";
import { readRecents } from "../lib/paletteRecents";
import { navigate } from "../web/router";
import { CommandPalette } from "../web/CommandPalette";
import { paletteEntries, type PaletteInput } from "../web/paletteEntries";
import type { ArtifactSummary, MemoryCardSummary, SharedWorkspace } from "../types";

const workspace = (id: string, name: string, role: SharedWorkspace["role"] = "owner"): SharedWorkspace => ({
  workspace: { id, name, created_at: "", updated_at: "", settings: {} },
  organization_id: "o1",
  role,
});
const file = (id: string, title: string, path = title) => ({ id, title, path }) as unknown as ArtifactSummary;
const card = (id: string, title: string) => ({ id, title }) as unknown as MemoryCardSummary;

const ATLAS = workspace("w1", "Atlas");
const BORROWED = workspace("w2", "Borrowed", "viewer");

const base: PaletteInput = {
  query: "",
  workspaces: [ATLAS, BORROWED],
  assistantWorkspace: undefined,
  currentWorkspace: undefined,
  files: [],
  cards: [],
  canAdministerSystem: false,
};

describe("paletteEntries", () => {
  it("offers a new note in the open workspace to anyone who can add evidence, and not to a viewer", () => {
    const writer = paletteEntries({ ...base, currentWorkspace: ATLAS });
    const entry = writer.find((candidate) => candidate.id === "new-note");
    expect(entry?.group).toBe("This workspace");
    expect(entry?.action).toEqual({ kind: "new-note", workspaceId: "w1", to: "/workspaces/w1/evidence" });
    expect(paletteEntries({ ...base, currentWorkspace: BORROWED }).some((candidate) => candidate.id === "new-note")).toBe(false);
    expect(paletteEntries(base).some((candidate) => candidate.id === "new-note")).toBe(false);
    expect(paletteEntries({ ...base, currentWorkspace: ATLAS, query: "note" }).some((candidate) => candidate.id === "new-note")).toBe(true);
    expect(paletteEntries({ ...base, currentWorkspace: ATLAS, query: "tasks" }).some((candidate) => candidate.id === "new-note")).toBe(false);
  });

  it("lists recent places first while nothing is typed, only of workspaces the person can still read", () => {
    const recents = [
      { id: "file:a1", label: "Retry policy", hint: "Atlas", to: "/workspaces/w1/artifacts/a1", workspaceId: "w1" },
      { id: "file:z9", label: "Lost file", hint: "Gone", to: "/workspaces/w9/artifacts/z9", workspaceId: "w9" },
    ];
    const empty = paletteEntries({ ...base, currentWorkspace: ATLAS, recents });
    expect(empty[0]).toMatchObject({ group: "Recent", label: "Retry policy", action: { kind: "navigate", to: "/workspaces/w1/artifacts/a1" } });
    expect(empty.filter((candidate) => candidate.group === "Recent")).toHaveLength(1);
    expect(paletteEntries({ ...base, currentWorkspace: ATLAS, recents, query: "retry" }).some((candidate) => candidate.group === "Recent")).toBe(false);
  });

  it("lists the account's pages and workspaces, and nothing of a workspace that is not open", () => {
    const entries = paletteEntries(base);
    expect(entries.filter((entry) => entry.group === "Workspaces").map((entry) => entry.label)).toEqual(["Atlas", "Borrowed"]);
    expect(entries.some((entry) => entry.group === "This workspace" || entry.group === "Assistant" || entry.group === "System")).toBe(false);
    expect(entries.find((entry) => entry.label === "Dashboard")?.action).toEqual({ kind: "navigate", to: "/dashboard" });
  });

  it("offers a workspace's sections, hiding administration from people who cannot administer it", () => {
    const owner = paletteEntries({ ...base, currentWorkspace: ATLAS }).filter((entry) => entry.group === "This workspace");
    expect(owner.map((entry) => entry.label)).toContain("People");
    expect(owner.map((entry) => entry.label)).toContain("Workspace settings");
    const viewer = paletteEntries({ ...base, currentWorkspace: BORROWED }).filter((entry) => entry.group === "This workspace");
    expect(viewer.map((entry) => entry.label)).not.toContain("People");
    expect(viewer.map((entry) => entry.label)).not.toContain("Workspace settings");
    expect(viewer.map((entry) => entry.label)).toContain("Retrieval");
  });

  it("finds files by every typed word in the title or path, and memory cards the server found", () => {
    const entries = paletteEntries({
      ...base,
      query: "retry policy",
      currentWorkspace: ATLAS,
      files: [file("a1", "Retry policy", "docs/retry-policy.md"), file("a2", "Deploy runbook"), file("a3", "Policy", "docs/policy.md")],
      cards: [card("c1", "Uploads retry with backoff")],
    });
    expect(entries.filter((entry) => entry.group === "Files").map((entry) => entry.label)).toEqual(["Retry policy"]);
    expect(entries.find((entry) => entry.group === "Files")?.action).toEqual({ kind: "navigate", to: "/workspaces/w1/artifacts/a1" });
    expect(entries.find((entry) => entry.group === "Memory")?.action).toEqual({ kind: "navigate", to: "/workspaces/w1/memory-cards/c1" });
  });

  it("starts assistant actions with what was typed, only where the assistant is docked", () => {
    const withoutDock = paletteEntries({ ...base, query: "how do retries work", currentWorkspace: ATLAS });
    expect(withoutDock.some((entry) => entry.group === "Assistant")).toBe(false);
    const entries = paletteEntries({ ...base, query: "how do retries work", currentWorkspace: ATLAS, assistantWorkspace: ATLAS });
    const first = entries[0];
    expect(first.group).toBe("Assistant");
    expect(first.action).toEqual({ kind: "assistant", capability: "ask_question", message: "how do retries work" });
    expect(entries.map((entry) => entry.action).filter((action) => action.kind === "assistant").length).toBeGreaterThanOrEqual(4);
  });

  it("offers the system pages only to administrators", () => {
    expect(paletteEntries({ ...base, query: "system" }).some((entry) => entry.group === "System")).toBe(false);
    const admin = paletteEntries({ ...base, query: "system users", canAdministerSystem: true });
    expect(admin.find((entry) => entry.group === "System")?.action).toEqual({ kind: "navigate", to: "/system/users" });
  });
});

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.mocked(querySharedArtifacts).mockResolvedValue([file("a1", "Retry policy", "docs/retry-policy.md")]);
    vi.mocked(searchSharedMemoryCards).mockResolvedValue([]);
    vi.mocked(navigate).mockClear();
    takeAssistantCommand();
  });
  afterEach(() => { cleanup(); window.localStorage.clear(); takeNewNoteRequest("w1"); });

  const palette = (assistantWorkspaceId: string | null) => render(
    <CommandPalette accessToken="t" assistantWorkspaceId={assistantWorkspaceId} canAdministerSystem={false} currentWorkspaceId="w1" workspaces={[ATLAS, BORROWED]} />,
  );

  it("opens with Ctrl K, filters as you type and opens the chosen file with Enter", async () => {
    palette(null);
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = await screen.findByRole("combobox");
    fireEvent.change(input, { target: { value: "retry" } });
    await waitFor(() => expect(screen.getByRole("option", { name: /Retry policy/ })).toBeTruthy());
    fireEvent.keyDown(input, { key: "Enter" });
    expect(navigate).toHaveBeenCalledWith("/workspaces/w1/artifacts/a1");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("remembers what was opened, and lists it first the next time, even after the page is loaded again", async () => {
    const { unmount } = render(<CommandPalette accessToken="t" assistantWorkspaceId={null} canAdministerSystem={false} currentWorkspaceId="w1" userId="u1" workspaces={[ATLAS, BORROWED]} />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    fireEvent.change(await screen.findByRole("combobox"), { target: { value: "retry" } });
    fireEvent.click(await screen.findByRole("option", { name: /Retry policy/ }));
    expect(navigate).toHaveBeenCalledWith("/workspaces/w1/artifacts/a1");
    expect(readRecents("u1").map((entry) => entry.label)).toEqual(["Retry policy"]);
    // A page loaded again starts from the browser's storage.
    unmount();
    render(<CommandPalette accessToken="t" assistantWorkspaceId={null} canAdministerSystem={false} currentWorkspaceId="w1" userId="u1" workspaces={[ATLAS, BORROWED]} />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const options = await screen.findAllByRole("option");
    expect(options[0].textContent).toContain("Retry policy");
    expect(screen.getByText("Recent")).toBeTruthy();
    // Another account on this browser has none.
    cleanup();
    render(<CommandPalette accessToken="t" assistantWorkspaceId={null} canAdministerSystem={false} currentWorkspaceId="w1" userId="u2" workspaces={[ATLAS, BORROWED]} />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    await screen.findAllByRole("option");
    expect(screen.queryByText("Recent")).toBeNull();
  });

  it("opens the note editor of the open workspace from New note", async () => {
    palette(null);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    fireEvent.change(await screen.findByRole("combobox"), { target: { value: "new note" } });
    fireEvent.click(await screen.findByRole("option", { name: /New note/ }));
    expect(navigate).toHaveBeenCalledWith("/workspaces/w1/evidence");
    // The evidence page takes the request once, for its own workspace only.
    expect(takeNewNoteRequest("w2")).toBe(false);
    expect(takeNewNoteRequest("w1")).toBe(true);
    expect(takeNewNoteRequest("w1")).toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("moves with the arrow keys and closes with Escape", async () => {
    palette(null);
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    const input = await screen.findByRole("combobox");
    const options = () => screen.getAllByRole("option");
    expect(options()[0].getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(options()[1].getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(options()[0].getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("hands an assistant action to the assistant of the page", async () => {
    palette("w1");
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = await screen.findByRole("combobox");
    fireEvent.change(input, { target: { value: "how do retries work" } });
    await waitFor(() => expect(screen.getByRole("option", { name: /Ask the assistant/ })).toBeTruthy());
    fireEvent.keyDown(input, { key: "Enter" });
    expect(takeAssistantCommand()).toEqual({ capability: "ask_question", message: "how do retries work" });
    expect(navigate).not.toHaveBeenCalled();
  });

  it("uses the shared search field for the query and the shared empty state when nothing matches", async () => {
    palette(null);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = await screen.findByRole("combobox");
    expect(input.closest(".rm-search-field")).not.toBeNull();
    fireEvent.change(input, { target: { value: "zzqxv" } });
    expect((await screen.findByText("Nothing matches “zzqxv”")).closest(".rm-empty-state")).not.toBeNull();
    expect(screen.queryAllByRole("option")).toHaveLength(0);
  });
});
