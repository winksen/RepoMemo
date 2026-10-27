import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const artifacts = vi.hoisted(() => ({ list: [] as unknown[] }));
vi.mock("../lib/sharedApi", () => ({
  bulkSharedArtifacts: vi.fn(),
  createSharedFolder: vi.fn(),
  createSharedTextArtifact: vi.fn(),
  deleteSharedArtifact: vi.fn(),
  deleteSharedFolder: vi.fn(),
  moveSharedArtifact: vi.fn(),
  moveSharedFolder: vi.fn(),
  renameSharedFolder: vi.fn(),
  uploadSharedArtifact: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));
vi.mock("../web/workspace/data", () => ({
  useEvidencePage: () => [{ items: artifacts.list, total: artifacts.list.length }],
  useFolders: () => [[], vi.fn()],
  useIndexFailures: () => [{}],
  useTemplate: () => [null],
}));
vi.mock("../web/lazy", () => ({
  DocumentViewer: () => null,
  RichNoteEditor: () => <div>Note editor</div>,
}));

import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { bulkSharedArtifacts } from "../lib/sharedApi";
import { EvidenceSection } from "../web/workspace/EvidenceSection";
import { makePage } from "./workspacePage";

afterEach(() => { cleanup(); artifacts.list = []; });

const file = { id: "a1", title: "Runbook", path: "docs/runbook.md", artifact_type: "markdown", size_bytes: 120, indexed_at: "2026-10-01T00:00:00Z", folder_id: null, repository_id: null };

describe("the evidence section on the shared pieces", () => {
  it("shows the shared empty state when the ledger has nothing", () => {
    render(<ConfirmProvider><EvidenceSection page={makePage()} /></ConfirmProvider>);
    expect(screen.getByText("No shared evidence yet").closest(".rm-empty-state")).not.toBeNull();
  });

  it("opens the folder form with a labelled, required field", () => {
    render(<ConfirmProvider><EvidenceSection page={makePage()} /></ConfirmProvider>);
    fireEvent.click(screen.getByRole("button", { name: /New folder/ }));
    const name = screen.getByLabelText("Folder name") as HTMLInputElement;
    expect(name.required).toBe(true);
    expect(name.closest(".rm-field")).not.toBeNull();
  });

  it("labels the note title and picks files with the shared checkbox while selecting", () => {
    artifacts.list = [file];
    render(<ConfirmProvider><EvidenceSection page={makePage()} /></ConfirmProvider>);
    expect((screen.getByLabelText("Note title") as HTMLInputElement).required).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Select files" }));
    const box = screen.getByRole("checkbox", { name: "Select Runbook" });
    expect(box.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(box);
    expect(screen.getByRole("checkbox", { name: "Select Runbook" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByText("1 selected")).toBeTruthy();
  });

  it("previews a bulk review and reports each result", async () => {
    artifacts.list = [file, { ...file, id: "a2", title: "Design", path: "docs/design.md" }];
    vi.mocked(bulkSharedArtifacts).mockResolvedValue({ done: ["a1"], failed: [{ artifact_id: "a2", message: "Artifact was not found." }] });
    render(<ConfirmProvider><EvidenceSection page={makePage()} /></ConfirmProvider>);
    fireEvent.click(screen.getByRole("button", { name: "Select files" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Select Runbook" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Select Design" }));
    fireEvent.click(screen.getByRole("button", { name: "Mark verified" }));
    expect(screen.getByRole("dialog", { name: "Review selected evidence" })).toBeTruthy();
    expect(bulkSharedArtifacts).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Mark 2 items verified" }));
    await waitFor(() => expect(bulkSharedArtifacts).toHaveBeenCalledWith("t", "w1", { action: "lifecycle", status: "verified", artifactIds: ["a1", "a2"] }));
    expect(await screen.findByText("Failed: Artifact was not found.")).toBeTruthy();
    expect(screen.getByText("Updated")).toBeTruthy();
    expect(screen.getByText("1 selected")).toBeTruthy();
  });
});
