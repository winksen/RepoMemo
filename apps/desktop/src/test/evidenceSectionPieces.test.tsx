import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
  useArtifacts: () => [artifacts.list, vi.fn()],
  useFolders: () => [[], vi.fn()],
  useIndexFailures: () => [{}],
}));
vi.mock("../web/lazy", () => ({
  DocumentViewer: () => null,
  RichNoteEditor: () => <div>Note editor</div>,
}));

import { EvidenceSection } from "../web/workspace/EvidenceSection";
import { makePage } from "./workspacePage";

afterEach(() => { cleanup(); artifacts.list = []; });

const file = { id: "a1", title: "Runbook", path: "docs/runbook.md", artifact_type: "markdown", size_bytes: 120, indexed_at: "2026-10-01T00:00:00Z", folder_id: null, repository_id: null };

describe("the evidence section on the shared pieces", () => {
  it("shows the shared empty state when the ledger has nothing", () => {
    render(<EvidenceSection page={makePage()} />);
    expect(screen.getByText("No shared evidence yet").closest(".rm-empty-state")).not.toBeNull();
  });

  it("opens the folder form with a labelled, required field", () => {
    render(<EvidenceSection page={makePage()} />);
    fireEvent.click(screen.getByRole("button", { name: /New folder/ }));
    const name = screen.getByLabelText("Folder name") as HTMLInputElement;
    expect(name.required).toBe(true);
    expect(name.closest(".rm-field")).not.toBeNull();
  });

  it("labels the note title and picks files with the shared checkbox while selecting", () => {
    artifacts.list = [file];
    render(<EvidenceSection page={makePage()} />);
    expect((screen.getByLabelText("Note title") as HTMLInputElement).required).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Select files" }));
    const box = screen.getByRole("checkbox", { name: "Select Runbook" });
    expect(box.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(box);
    expect(screen.getByRole("checkbox", { name: "Select Runbook" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByText("1 selected")).toBeTruthy();
  });
});
