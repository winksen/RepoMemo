import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  compareSharedArtifactVersions: vi.fn(),
  getSharedArtifactVersionText: vi.fn(),
  listSharedArtifactVersions: vi.fn(),
  restoreSharedArtifactVersion: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { compareSharedArtifactVersions, getSharedArtifactVersionText, listSharedArtifactVersions, restoreSharedArtifactVersion } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { ArtifactVersionsPanel } from "../components/ArtifactVersionsPanel";
import type { ArtifactVersion } from "../types";

const version = (number: number, overrides: Partial<ArtifactVersion> = {}): ArtifactVersion => ({
  version: number,
  content_hash: `hash-${number}`,
  size_bytes: 1024 * number,
  created_at: "2026-10-01T10:00:00Z",
  uploaded_by: number > 1 ? "Ada" : null,
  replaced_at: null,
  is_current: false,
  ...overrides,
});

const history = [version(3, { is_current: true }), version(2, { replaced_at: "2026-10-03T10:00:00Z" }), version(1, { replaced_at: "2026-10-02T10:00:00Z" })];

function panel(extra: { canWrite?: boolean; onRestored?: () => void } = {}) {
  return render(<ConfirmProvider><ArtifactVersionsPanel accessToken="t" artifactId="a1" updatedAt="2026-10-03T10:00:00Z" {...extra} /></ConfirmProvider>);
}

describe("ArtifactVersionsPanel", () => {
  beforeEach(() => {
    vi.mocked(listSharedArtifactVersions).mockResolvedValue(history);
  });
  afterEach(() => cleanup());

  it("shows nothing for a file that was never replaced", async () => {
    vi.mocked(listSharedArtifactVersions).mockResolvedValue([version(1, { is_current: true })]);
    const { container } = panel();
    await waitFor(() => expect(listSharedArtifactVersions).toHaveBeenCalled());
    expect(container.querySelector(".shared-versions-panel")).toBeNull();
  });

  it("lists the versions, current first, and offers what each allows", async () => {
    panel();
    await screen.findByText("Version 3 · current");
    expect(screen.getByText("Version 2")).toBeTruthy();
    expect(screen.getByText("Version 1")).toBeTruthy();
    // Every version can be read; all but the first say what changed; all but the current compare with it.
    expect(screen.getAllByRole("button", { name: "View text" }).length).toBe(3);
    expect(screen.getAllByRole("button", { name: "What changed" }).length).toBe(2);
    expect(screen.getAllByRole("button", { name: "Compare with current" }).length).toBe(2);
  });

  it("opens the text of an earlier version", async () => {
    vi.mocked(getSharedArtifactVersionText).mockResolvedValue({ version: 1, text: "First edition.", truncated: false, note: null });
    panel();
    const buttons = await screen.findAllByRole("button", { name: "View text" });
    fireEvent.click(buttons[2]);
    await screen.findByText("First edition.");
    expect(getSharedArtifactVersionText).toHaveBeenCalledWith("t", "a1", 1);
  });

  it("shows a comparison with the changed lines marked", async () => {
    vi.mocked(compareSharedArtifactVersions).mockResolvedValue({
      from_version: 1,
      to_version: 3,
      added: 1,
      removed: 1,
      truncated: false,
      note: null,
      lines: [
        { kind: "same", text: "# Guide", old_line: 1, new_line: 1 },
        { kind: "removed", text: "First edition.", old_line: 2, new_line: null },
        { kind: "added", text: "Second edition.", old_line: null, new_line: 2 },
        { kind: "gap", text: "12 unchanged lines", old_line: null, new_line: null },
      ],
    });
    panel();
    const buttons = await screen.findAllByRole("button", { name: "Compare with current" });
    fireEvent.click(buttons[1]);
    await screen.findByText("First edition.");
    expect(compareSharedArtifactVersions).toHaveBeenCalledWith("t", "a1", 1, 3);
    expect(screen.getByText("Second edition.")).toBeTruthy();
    expect(screen.getByText("… 12 unchanged lines")).toBeTruthy();
    expect(screen.getByText("1 line removed, 1 added")).toBeTruthy();
  });

  it("offers no restore to someone who cannot change the file", async () => {
    panel();
    await screen.findByText("Version 3 · current");
    expect(screen.queryByRole("button", { name: /Restore version/ })).toBeNull();
  });

  it("offers a restore for every earlier version but the current one", async () => {
    panel({ canWrite: true });
    await screen.findByText("Version 3 · current");
    expect(screen.getByRole("button", { name: "Restore version 2" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Restore version 1" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Restore version 3" })).toBeNull();
  });

  it("asks first, restores the chosen version as a new one and tells the page", async () => {
    vi.mocked(restoreSharedArtifactVersion).mockResolvedValue({} as never);
    const onRestored = vi.fn();
    panel({ canWrite: true, onRestored });
    fireEvent.click(await screen.findByRole("button", { name: "Restore version 2" }));
    // The question says what will happen, and nothing is sent before the answer.
    expect(await screen.findByText(/Version 2 becomes version 4/)).toBeTruthy();
    expect(restoreSharedArtifactVersion).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Restore" }));
    await waitFor(() => expect(restoreSharedArtifactVersion).toHaveBeenCalledWith("t", "a1", 2));
    await waitFor(() => expect(onRestored).toHaveBeenCalledTimes(1));
    expect(showToast).toHaveBeenCalledWith("success", expect.stringContaining("Version 2 restored as version 4"));
  });

  it("sends nothing when the question is cancelled, and says why when the server refuses", async () => {
    panel({ canWrite: true });
    fireEvent.click(await screen.findByRole("button", { name: "Restore version 1" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByText(/Version 1 becomes version 4/)).toBeNull());
    expect(restoreSharedArtifactVersion).not.toHaveBeenCalled();

    vi.mocked(restoreSharedArtifactVersion).mockRejectedValueOnce(new Error("That is already the current version of this file."));
    fireEvent.click(screen.getByRole("button", { name: "Restore version 1" }));
    fireEvent.click(await screen.findByRole("button", { name: "Restore" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("already the current version")));
  });
});
