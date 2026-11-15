import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ previewSharedFolderMove: vi.fn() }));

import { previewSharedFolderMove } from "../lib/sharedApi";
import { FolderMovePreviewNote, folderMoveSummary } from "../components/FolderMovePreviewNote";
import type { FolderMovePreview } from "../types";

const preview = (overrides: Partial<FolderMovePreview> = {}): FolderMovePreview => ({
  folder_name: "Policies", from_path: "Policies", to_path: "Archive / Policies", subfolders: 1, files: 3, citing_cards: 2, citations: 4, comments: 1, unchanged: false, allowed: true, refusal: null, ...overrides,
});

describe("the folder move preview", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(cleanup);

  it("says where the folder ends up, what is inside and that citations and comments stay attached", () => {
    const text = folderMoveSummary(preview());
    expect(text).toContain("Policies becomes Archive / Policies, with 3 files and 1 folder inside it.");
    expect(text).toContain("2 memory cards cite these files (4 citations)");
    expect(text).toContain("they all stay attached");
  });

  it("says when nothing is attached, and when the folder is already there", () => {
    expect(folderMoveSummary(preview({ citations: 0, citing_cards: 0, comments: 0 }))).toContain("Nothing cites these files");
    expect(folderMoveSummary(preview({ unchanged: true }))).toBe("Policies is already there.");
  });

  it("gives the reason a move is refused instead of the preview", () => {
    expect(folderMoveSummary(preview({ allowed: false, refusal: "A folder cannot be moved into itself or into a folder inside it." }))).toBe("A folder cannot be moved into itself or into a folder inside it.");
  });

  it("shows that it is checking, then the answer, and says when it could not read", async () => {
    vi.mocked(previewSharedFolderMove).mockReturnValueOnce(new Promise(() => undefined));
    const first = render(<FolderMovePreviewNote accessToken="t" folderId="f1" parentId={null} workspaceId="w1" />);
    expect(screen.getByRole("status").textContent).toContain("Checking what the move would do");
    first.unmount();
    vi.mocked(previewSharedFolderMove).mockResolvedValueOnce(preview());
    const second = render(<FolderMovePreviewNote accessToken="t" folderId="f1" parentId="f2" workspaceId="w1" />);
    expect(await screen.findByText(/Policies becomes Archive \/ Policies/)).toBeTruthy();
    expect(previewSharedFolderMove).toHaveBeenLastCalledWith("t", "w1", "f1", "f2");
    second.unmount();
    vi.mocked(previewSharedFolderMove).mockRejectedValueOnce(new Error("You do not have access to this workspace."));
    render(<FolderMovePreviewNote accessToken="t" folderId="f1" parentId={null} workspaceId="w1" />);
    expect(await screen.findByText("You do not have access to this workspace.")).toBeTruthy();
  });
});
