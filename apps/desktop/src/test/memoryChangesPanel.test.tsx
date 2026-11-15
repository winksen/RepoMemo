import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ getSharedMemoryChanges: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { getSharedMemoryChanges } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { MemoryChangesPanel } from "../components/MemoryChangesPanel";
import type { MemoryChange, MemoryChanges } from "../types";

const change = (overrides: Partial<MemoryChange> = {}): MemoryChange => ({
  kind: "retired", artifact_id: "a1", title: "Refund policy", summary: "The file was retired.", comparison: null, unavailable_reason: "Only its status changed; its text did not.", ...overrides,
});
const changes = (list: MemoryChange[]): MemoryChanges => ({ revision: 2, changes: list });

function renderPanel(props: { canWrite?: boolean; isBusy?: boolean } = {}) {
  const onOpenArtifact = vi.fn();
  const onRemoveCitation = vi.fn();
  const view = render(<MemoryChangesPanel accessToken="t" canWrite={props.canWrite ?? true} cardId="c1" isBusy={props.isBusy ?? false} onOpenArtifact={onOpenArtifact} onRemoveCitation={onRemoveCitation} updatedAt="now" />);
  return { ...view, onOpenArtifact, onRemoveCitation };
}

describe("the card's What changed panel", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(cleanup);

  it("shows nothing while loading and nothing when no evidence changed", async () => {
    vi.mocked(getSharedMemoryChanges).mockResolvedValue(changes([]));
    const { container } = renderPanel();
    expect(container.textContent).toBe("");
    await waitFor(() => expect(getSharedMemoryChanges).toHaveBeenCalled());
    expect(container.textContent).toBe("");
  });

  it("says why no text is shown, and lets an editor open the file or remove the citation", async () => {
    vi.mocked(getSharedMemoryChanges).mockResolvedValue(changes([change()]));
    const { onOpenArtifact, onRemoveCitation } = renderPanel();
    expect(await screen.findByText("Only its status changed; its text did not.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open the file" }));
    expect(onOpenArtifact).toHaveBeenCalledWith("a1");
    fireEvent.click(screen.getByRole("button", { name: "Remove this citation" }));
    expect(onRemoveCitation).toHaveBeenCalledWith("a1");
  });

  it("names a missing citation as missing and shows no file to open", async () => {
    vi.mocked(getSharedMemoryChanges).mockResolvedValue(changes([change({ kind: "missing", artifact_id: null, unavailable_reason: "Removed text is not kept." })]));
    renderPanel();
    expect(await screen.findByRole("button", { name: "Remove the missing citation" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Open the file" })).toBeNull();
  });

  it("gives a reader the explanation without the buttons that change the card", async () => {
    vi.mocked(getSharedMemoryChanges).mockResolvedValue(changes([change()]));
    renderPanel({ canWrite: false });
    expect(await screen.findByText("The file was retired.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Remove this citation" })).toBeNull();
  });

  it("disables removal while the card is being saved", async () => {
    vi.mocked(getSharedMemoryChanges).mockResolvedValue(changes([change()]));
    renderPanel({ isBusy: true });
    expect((await screen.findByRole("button", { name: "Remove this citation" })).hasAttribute("disabled")).toBe(true);
  });

  it("says in a toast when access is refused instead of showing an empty panel as fact", async () => {
    vi.mocked(getSharedMemoryChanges).mockRejectedValue(new Error("You do not have access to this card."));
    renderPanel();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "You do not have access to this card."));
  });
});
