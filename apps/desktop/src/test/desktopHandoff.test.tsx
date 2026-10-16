import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn() }));
vi.mock("../lib/desktop", () => ({ isDesktopRuntime: true }));
vi.mock("../lib/repomemoApi", () => ({ listWorkspaces: vi.fn() }));
vi.mock("../lib/sharedApi", () => ({ importSharedWorkspaceArchive: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { listWorkspaces } from "../lib/repomemoApi";
import { importSharedWorkspaceArchive } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { loadPreparedArchive, pendingHandoff, prepareLocalArchive, rememberHandoff, type PreparedArchive } from "../lib/handoff";
import { LocalMoveToServer, describeCounts } from "../components/LocalMoveToServer";
import { WorkspaceImport } from "../components/WorkspaceImport";
import type { ArchiveImportReport } from "../types";

const counts = { sources: 1, folders: 0, artifacts: 4, blobs: 4, memory_cards: 2, citations: 3, tasks: 1, checklist_items: 0, comments: 0, lifecycle_events: 0, omitted_citations: 0 };
const prepared: PreparedArchive = { path: "C:\\data\\exports\\repomemo-notes.tar.gz", file_name: "repomemo-notes.tar.gz", workspace_name: "Notes", size_bytes: 2048, counts };

function report(changes: Partial<ArchiveImportReport> = {}): ArchiveImportReport {
  return {
    dry_run: true, ok: true, workspace_name: "Notes", archive_format_version: 1, source_version: "0.2.50",
    exported_at: "2026-10-16T10:00:00Z", counts, total_bytes: 2048, people: [], problems: [], warnings: [],
    widened_citations: 0, workspace: null, ...changes,
  };
}

beforeEach(() => {
  window.localStorage.clear();
  vi.mocked(invoke).mockReset();
  vi.mocked(showToast).mockReset();
  vi.mocked(importSharedWorkspaceArchive).mockReset();
});
afterEach(cleanup);

describe("handoff helpers", () => {
  it("ask the shell by workspace id and by the archive's own path", async () => {
    vi.mocked(invoke).mockResolvedValue(prepared);
    expect(await prepareLocalArchive("w1")).toEqual(prepared);
    expect(invoke).toHaveBeenCalledWith("export_workspace_archive", { workspaceId: "w1" });
  });

  it("turn the shell's bytes into a file under the archive's name", async () => {
    vi.mocked(invoke).mockResolvedValue(new Uint8Array([1, 2, 3]).buffer);
    const file = await loadPreparedArchive(prepared);
    expect(invoke).toHaveBeenCalledWith("read_export", { path: prepared.path });
    expect(file.name).toBe("repomemo-notes.tar.gz");
    expect(file.size).toBe(3);
  });

  it("remember one waiting archive, and ignore what is not one", () => {
    expect(pendingHandoff()).toBeNull();
    rememberHandoff(prepared);
    expect(pendingHandoff()).toEqual(prepared);
    rememberHandoff(null);
    expect(pendingHandoff()).toBeNull();
    window.localStorage.setItem("repomemo.desktop.handoff", "{\"path\":1}");
    expect(pendingHandoff()).toBeNull();
    window.localStorage.setItem("repomemo.desktop.handoff", "not json");
    expect(pendingHandoff()).toBeNull();
  });
});

describe("moving a local workspace", () => {
  it("says what the archive holds", () => {
    expect(describeCounts(prepared)).toBe("4 files, 0 folders, 2 memory cards, 3 citations, 1 task");
  });

  it("prepares the chosen workspace, then waits for the server with the archive remembered", async () => {
    vi.mocked(listWorkspaces).mockResolvedValue([{ id: "w1", name: "Notes", created_at: "", updated_at: "" } as never]);
    vi.mocked(invoke).mockResolvedValue(prepared);
    const onGoToServer = vi.fn();
    render(<LocalMoveToServer onGoToServer={onGoToServer} />);

    fireEvent.click(screen.getByRole("button", { name: /Move a workspace to a server/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Prepare the archive/ }));
    expect(await screen.findByText(/Notes is ready: 4 files/)).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("export_workspace_archive", { workspaceId: "w1" });
    expect(pendingHandoff()).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /Go to the server to import it/ }));
    expect(onGoToServer).toHaveBeenCalledTimes(1);
    expect(pendingHandoff()).toEqual(prepared);
  });

  it("saves a copy where the person chooses, and deletes the archive when it is discarded", async () => {
    vi.mocked(listWorkspaces).mockResolvedValue([{ id: "w1", name: "Notes" } as never]);
    vi.mocked(invoke).mockResolvedValue(prepared);
    vi.mocked(save).mockResolvedValue("D:\\backup\\notes.tar.gz");
    render(<LocalMoveToServer onGoToServer={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: /Move a workspace to a server/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Prepare the archive/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Save a copy/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("copy_export", { path: prepared.path, destination: "D:\\backup\\notes.tar.gz" }));

    fireEvent.click(screen.getByRole("button", { name: /Discard the archive/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("discard_export", { path: prepared.path }));
    expect(pendingHandoff()).toBeNull();
  });

  it("shows the shell's own words when the workspace cannot be prepared", async () => {
    vi.mocked(listWorkspaces).mockResolvedValue([{ id: "w1", name: "Notes" } as never]);
    vi.mocked(invoke).mockRejectedValue("1 file(s) of this workspace have no stored content.");
    render(<LocalMoveToServer onGoToServer={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: /Move a workspace to a server/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Prepare the archive/ }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "1 file(s) of this workspace have no stored content."));
  });
});

describe("importing the prepared archive on the server", () => {
  it("checks it as soon as the page shows, imports it when asked and then deletes it", async () => {
    rememberHandoff(prepared);
    vi.mocked(invoke).mockImplementation(async (command) => (command === "read_export" ? new Uint8Array([7, 7]).buffer : undefined));
    const created = { workspace: { id: "w9", name: "Notes", created_at: "", updated_at: "", settings: {} }, organization_id: "o1", role: "owner" } as never;
    vi.mocked(importSharedWorkspaceArchive)
      .mockResolvedValueOnce(report())
      .mockResolvedValueOnce(report({ dry_run: false, workspace: created }));
    const onImported = vi.fn();
    render(<WorkspaceImport accessToken="t" onImported={onImported} organizationId="o1" />);

    expect(await screen.findByText(/Prepared on this computer/)).toBeTruthy();
    await screen.findByText(/4 files, 0 folders, 2 memory cards/);
    const [, firstCall] = vi.mocked(importSharedWorkspaceArchive).mock.calls[0];
    expect(firstCall).toMatchObject({ organizationId: "o1", dryRun: true });
    expect((firstCall.archive as File).name).toBe("repomemo-notes.tar.gz");
    expect(onImported).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /import workspace/i }));
    await waitFor(() => expect(onImported).toHaveBeenCalledWith(created));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("discard_export", { path: prepared.path }));
    expect(pendingHandoff()).toBeNull();
    expect(screen.queryByText(/Prepared on this computer/)).toBeNull();
  });

  it("lets the person discard the prepared archive instead", async () => {
    rememberHandoff(prepared);
    vi.mocked(invoke).mockImplementation(async (command) => (command === "read_export" ? new Uint8Array([7]).buffer : undefined));
    vi.mocked(importSharedWorkspaceArchive).mockResolvedValue(report());
    render(<WorkspaceImport accessToken="t" onImported={() => undefined} organizationId="o1" />);
    fireEvent.click(await screen.findByRole("button", { name: /Discard it/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("discard_export", { path: prepared.path }));
    expect(pendingHandoff()).toBeNull();
    expect(screen.queryByText(/Prepared on this computer/)).toBeNull();
  });
});
