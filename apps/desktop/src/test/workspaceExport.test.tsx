import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ downloadSharedWorkspaceArchive: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { downloadSharedWorkspaceArchive } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { WorkspaceExportSettings } from "../components/WorkspaceExportSettings";

describe("WorkspaceExportSettings", () => {
  beforeEach(() => {
    vi.mocked(downloadSharedWorkspaceArchive).mockReset();
    vi.mocked(showToast).mockReset();
    URL.createObjectURL = vi.fn(() => "blob:archive");
    URL.revokeObjectURL = vi.fn();
  });
  afterEach(() => cleanup());

  it("downloads the archive under the name the server gave it and says so", async () => {
    vi.mocked(downloadSharedWorkspaceArchive).mockResolvedValue({ blob: new Blob(["x"]), filename: "repomemo-docs-20261016.tar.gz" });
    const clicked: string[] = [];
    const click = HTMLAnchorElement.prototype.click;
    HTMLAnchorElement.prototype.click = function () { clicked.push(this.download); };
    try {
      render(<WorkspaceExportSettings accessToken="t" workspaceId="w1" />);
      fireEvent.click(screen.getByRole("button", { name: /download archive/i }));
      await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", "The workspace archive was downloaded."));
      expect(downloadSharedWorkspaceArchive).toHaveBeenCalledWith("t", "w1");
      expect(clicked).toEqual(["repomemo-docs-20261016.tar.gz"]);
    } finally { HTMLAnchorElement.prototype.click = click; }
  });

  it("shows the server's reason as a toast when the export is refused", async () => {
    vi.mocked(downloadSharedWorkspaceArchive).mockRejectedValue(new Error("You do not have access to this workspace."));
    render(<WorkspaceExportSettings accessToken="t" workspaceId="w1" />);
    fireEvent.click(screen.getByRole("button", { name: /download archive/i }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "You do not have access to this workspace."));
    expect((screen.getByRole("button", { name: /download archive/i }) as HTMLButtonElement).disabled).toBe(false);
  });
});
