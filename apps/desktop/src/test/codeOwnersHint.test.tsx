import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ getSharedArtifactCodeOwners: vi.fn(), requestSharedArtifactReview: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { getSharedArtifactCodeOwners, requestSharedArtifactReview } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { CodeOwnersHint } from "../components/CodeOwnersHint";
import type { CodeOwnerMatch } from "../types";

const hints = ["ada@example.com", "@payments", "nobody@example.com"];
const matches: CodeOwnerMatch[] = [
  { owner: "ada@example.com", member: { id: "u1", display_name: "Ada Lovelace", email: "ada@example.com" } },
  { owner: "@payments", member: null },
  { owner: "nobody@example.com", member: null },
];

function hint(canWrite = true) {
  return render(<span><CodeOwnersHint accessToken="t" artifactId="a1" canWrite={canWrite} hints={hints} /></span>);
}

beforeEach(() => { vi.mocked(getSharedArtifactCodeOwners).mockResolvedValue(matches); });
afterEach(cleanup);

describe("CodeOwnersHint", () => {
  it("shows nothing for a file with no owners, and asks the server nothing", () => {
    const { container } = render(<CodeOwnersHint accessToken="t" artifactId="a1" canWrite hints={[]} />);
    expect(container.textContent).toBe("");
    expect(getSharedArtifactCodeOwners).not.toHaveBeenCalled();
  });

  it("starts as the hints it always was, then shows a matched owner as the member, with the others still hints", async () => {
    hint();
    expect(screen.getByText(/CODEOWNERS suggests/).textContent).toContain("ada@example.com");
    await screen.findByText("Ada Lovelace");
    expect(screen.getByText("(ada@example.com)")).toBeTruthy();
    expect(screen.getByText("@payments")).toBeTruthy();
    expect(screen.getByText("nobody@example.com")).toBeTruthy();
    // Only the matched owner can be asked.
    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Ask Ada Lovelace to review this file" })).toBeTruthy();
  });

  it("keeps the hints when the server cannot say, and offers nobody to ask", async () => {
    vi.mocked(getSharedArtifactCodeOwners).mockRejectedValue(new Error("down"));
    hint();
    await waitFor(() => expect(getSharedArtifactCodeOwners).toHaveBeenCalled());
    expect(screen.getByText(/CODEOWNERS suggests/).textContent).toContain("@payments");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("offers no request to someone who cannot change the file", async () => {
    hint(false);
    await screen.findByText("Ada Lovelace");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("asks a matched owner to review, once, and says so", async () => {
    vi.mocked(requestSharedArtifactReview).mockResolvedValue({} as never);
    hint();
    fireEvent.click(await screen.findByRole("button", { name: "Ask Ada Lovelace to review this file" }));
    await waitFor(() => expect(requestSharedArtifactReview).toHaveBeenCalledWith("t", "a1", "u1"));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", "Ada Lovelace was asked to review this file."));
    const button = await screen.findByRole("button", { name: "Ask Ada Lovelace to review this file" });
    expect(button.textContent).toBe("Asked");
    expect(button.hasAttribute("disabled")).toBe(true);
  });

  it("says why a request was refused and lets it be tried again", async () => {
    vi.mocked(requestSharedArtifactReview).mockRejectedValue(new Error("This person has already been asked to review this file, and has not finished."));
    hint();
    fireEvent.click(await screen.findByRole("button", { name: "Ask Ada Lovelace to review this file" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("already been asked")));
    expect(screen.getByRole("button", { name: "Ask Ada Lovelace to review this file" }).textContent).toBe("Ask to review");
  });
});
