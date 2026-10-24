import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ getSharedArtifactAtCommit: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { getSharedArtifactAtCommit } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { ViewAtCommit } from "../web/common";
import type { RepoFileAtCommit } from "../types";

const COMMIT = "0123456789abcdef0123456789abcdef01234567";
const file = (overrides: Partial<RepoFileAtCommit> = {}): RepoFileAtCommit => ({
  artifact_id: "a1", path: "src/main.rs", commit: COMMIT, text: "fn main() {\n    run();\n}\n", summary: "Start the run", author_name: "Ada", committed_at: "2026-10-01T10:00:00Z",
  current_commit: "fedcba9876543210fedcba9876543210fedcba98", ...overrides,
} as RepoFileAtCommit);

afterEach(cleanup);

describe("ViewAtCommit", () => {
  it("offers nothing for a link that names no commit (a card that did not say, a file that is not a repository's)", () => {
    const { container } = render(<ViewAtCommit accessToken="t" citation={{ artifact_id: "a1", commit: null, end_line: null, path: "notes.md", start_line: null }} />);
    expect(container.textContent).toBe("");
    cleanup();
    const missing = render(<ViewAtCommit accessToken="t" citation={{ artifact_id: "a1", end_line: null, path: "notes.md", start_line: null }} />);
    expect(missing.container.textContent).toBe("");
  });

  it("opens the cited text at the commit a memory card's link names, labelled as such, with its lines marked", async () => {
    vi.mocked(getSharedArtifactAtCommit).mockResolvedValue(file());
    render(<ViewAtCommit accessToken="t" citation={{ artifact_id: "a1", commit: COMMIT, end_line: 2, path: "src/main.rs", start_line: 2 }} />);
    const button = screen.getByRole("button", { name: "View at 0123456" });
    expect(button.getAttribute("title")).toContain("as it was at commit 0123456");
    fireEvent.click(button);
    await screen.findByRole("dialog");
    expect(getSharedArtifactAtCommit).toHaveBeenCalledWith("t", "a1", COMMIT);
    expect(screen.getByText(/As it was at commit/).textContent).toContain("0123456");
    expect(screen.getByText(/not the current version/)).toBeTruthy();
    // Only the cited line is marked.
    const marked = Array.from(document.querySelectorAll(".shared-commit-text mark")).map((mark) => mark.textContent);
    expect(marked).toEqual(["    run();"]);
  });

  it("says when the commit is also the current version, and why it could not be read", async () => {
    vi.mocked(getSharedArtifactAtCommit).mockResolvedValueOnce(file({ current_commit: null }));
    render(<ViewAtCommit accessToken="t" citation={{ artifact_id: "a1", commit: COMMIT, end_line: null, path: "src/main.rs", start_line: null }} />);
    fireEvent.click(screen.getByRole("button", { name: /View at/ }));
    expect(await screen.findByText(/This is also the current version/)).toBeTruthy();
    cleanup();
    vi.mocked(getSharedArtifactAtCommit).mockRejectedValueOnce(new Error("That commit is no longer in the repository's copy."));
    render(<ViewAtCommit accessToken="t" citation={{ artifact_id: "a1", commit: COMMIT, end_line: null, path: "src/main.rs", start_line: null }} />);
    fireEvent.click(screen.getByRole("button", { name: /View at/ }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("no longer in the repository")));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
