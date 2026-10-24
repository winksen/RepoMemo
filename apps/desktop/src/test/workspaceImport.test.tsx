import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ importSharedWorkspaceArchive: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { importSharedWorkspaceArchive } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { WorkspaceImport, describeArchive } from "../components/WorkspaceImport";
import type { ArchiveImportReport } from "../types";

const counts = { sources: 1, folders: 1, artifacts: 3, blobs: 2, memory_cards: 1, citations: 2, tasks: 1, checklist_items: 0, comments: 2, lifecycle_events: 1, omitted_citations: 0 };

function report(changes: Partial<ArchiveImportReport> = {}): ArchiveImportReport {
  return {
    dry_run: true, ok: true, workspace_name: "Payments", archive_format_version: 1, source_version: "0.2.41",
    exported_at: "2026-10-16T10:00:00Z", counts, total_bytes: 2048, people: [], problems: [], warnings: [],
    widened_citations: 0, workspace: null, ...changes,
  };
}

function choose(file: File) {
  fireEvent.change(screen.getByLabelText("Workspace archive"), { target: { files: [file] } });
}

describe("describeArchive", () => {
  it("says what the archive holds in the singular and the plural", () => {
    expect(describeArchive(report())).toBe("3 files, 1 folder, 1 memory card, 1 task, 2 comments");
  });

  it("also names the earlier versions and the saved searches of an archive that carries them, and nothing for one that does not", () => {
    const withHistory = report();
    withHistory.counts = { ...withHistory.counts, versions: 4, saved_searches: 1 };
    expect(describeArchive(withHistory)).toBe("3 files, 1 folder, 1 memory card, 1 task, 2 comments, 4 earlier versions, 1 saved search");
    const none = report();
    none.counts = { ...none.counts, versions: 0, saved_searches: 0 };
    expect(describeArchive(none)).toBe("3 files, 1 folder, 1 memory card, 1 task, 2 comments");
  });
});

describe("WorkspaceImport", () => {
  beforeEach(() => {
    vi.mocked(importSharedWorkspaceArchive).mockReset();
    vi.mocked(showToast).mockReset();
  });
  afterEach(() => cleanup());

  const archive = () => new File(["x"], "payments.tar.gz", { type: "application/gzip" });

  it("checks the archive first and imports only when asked, under the name chosen", async () => {
    const created = { workspace: { id: "w9", name: "Payments copy", created_at: "", updated_at: "", settings: {} }, organization_id: "o1", role: "owner" } as never;
    vi.mocked(importSharedWorkspaceArchive)
      .mockResolvedValueOnce(report())
      .mockResolvedValueOnce(report({ dry_run: false, workspace: created }));
    const onImported = vi.fn();
    render(<WorkspaceImport accessToken="t" onImported={onImported} organizationId="o1" />);

    expect((screen.getByRole("button", { name: /check the archive/i }) as HTMLButtonElement).disabled).toBe(true);
    const file = archive();
    choose(file);
    fireEvent.click(screen.getByRole("button", { name: /check the archive/i }));
    await screen.findByText("Payments");
    expect(importSharedWorkspaceArchive).toHaveBeenLastCalledWith("t", { organizationId: "o1", archive: file, dryRun: true, name: "" });
    expect(screen.getByText(/3 files, 1 folder/)).toBeTruthy();
    expect(onImported).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText(/name of the new workspace/i), { target: { value: "Payments copy" } });
    fireEvent.click(screen.getByRole("button", { name: /import workspace/i }));
    await waitFor(() => expect(onImported).toHaveBeenCalledWith(created));
    expect(importSharedWorkspaceArchive).toHaveBeenLastCalledWith("t", { organizationId: "o1", archive: file, dryRun: false, name: "Payments copy" });
    expect(showToast).toHaveBeenCalledWith("success", "Imported Payments copy.");
    expect(screen.queryByText("Payments")).toBeNull();
  });

  it("lists the problems of an archive that cannot be imported and offers no import", async () => {
    vi.mocked(importSharedWorkspaceArchive).mockResolvedValue(report({ ok: false, problems: ["A stored file does not match its checksum."], warnings: ["1 comment will be left out."] }));
    render(<WorkspaceImport accessToken="t" onImported={vi.fn()} organizationId="o1" />);
    choose(archive());
    fireEvent.click(screen.getByRole("button", { name: /check the archive/i }));
    await screen.findByText("A stored file does not match its checksum.");
    expect(screen.getByText("1 comment will be left out.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /import workspace/i })).toBeNull();
    expect(screen.getByText("This archive cannot be imported.")).toBeTruthy();
  });

  it("shows the server's reason as a toast when the file is refused", async () => {
    vi.mocked(importSharedWorkspaceArchive).mockRejectedValue(new Error("This file is not a readable workspace archive."));
    render(<WorkspaceImport accessToken="t" onImported={vi.fn()} organizationId="o1" />);
    choose(archive());
    fireEvent.click(screen.getByRole("button", { name: /check the archive/i }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "This file is not a readable workspace archive."));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows how far the import has got while it runs, and takes the bar away when it ends", async () => {
    let finish: (value: ArchiveImportReport) => void = () => {};
    vi.mocked(importSharedWorkspaceArchive).mockImplementation((_token, input) => new Promise((resolve) => {
      input.onProgress?.({ stage: "storing_files", current: 512, total: 1024 });
      finish = resolve;
    }));
    render(<WorkspaceImport accessToken="t" onImported={vi.fn()} organizationId="o1" />);
    choose(archive());
    fireEvent.click(screen.getByRole("button", { name: /check the archive/i }));
    const bar = await screen.findByRole("progressbar", { name: "Import progress" }) as HTMLProgressElement;
    expect(bar.value).toBe(512);
    expect(bar.max).toBe(1024);
    expect(screen.getByText(/Storing the files: 50% of/)).toBeTruthy();
    finish(report());
    await screen.findByText("Payments");
    expect(screen.queryByRole("progressbar")).toBeNull();
  });
});
