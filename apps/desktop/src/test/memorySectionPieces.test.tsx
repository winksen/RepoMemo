import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  createSharedMemoryCard: vi.fn(),
  searchSharedMemoryCards: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));
vi.mock("../web/workspace/data", () => ({
  useArtifactPicker: () => {
    const files = [{ id: "a1", title: "Runbook", path: "docs/runbook.md" }, { id: "a2", title: "Design notes", path: "docs/design.md" }];
    return { files, search: () => undefined, find: (id: string) => files.find((entry) => entry.id === id), known: () => files };
  },
  useMemoryCards: () => [[]],
}));

import { MemorySection } from "../web/workspace/MemorySection";
import { makePage } from "./workspacePage";

afterEach(() => { cleanup(); });

describe("the memory section on the shared pieces", () => {
  it("uses the shared search field and labelled, required fields for a new card", () => {
    render(<MemorySection page={makePage({ section: "memory" })} />);
    expect(screen.getByLabelText("Search team memory").closest(".rm-search-field")).not.toBeNull();
    expect((screen.getByLabelText("Title") as HTMLInputElement).required).toBe(true);
    expect((screen.getByLabelText("Statement") as HTMLTextAreaElement).required).toBe(true);
    expect(screen.getByText("No durable memory cards yet").closest(".rm-empty-state")).not.toBeNull();
  });

  it("links evidence by typing part of a file name in the picker and lists the link as a chip", async () => {
    render(<MemorySection page={makePage({ section: "memory" })} />);
    fireEvent.click(screen.getByRole("button", { name: "Add an evidence link" }));
    fireEvent.change(await screen.findByPlaceholderText("Type to search"), { target: { value: "design" } });
    fireEvent.click(await screen.findByRole("option", { name: /Design notes/ }));
    expect(screen.getByRole("list", { name: "Linked evidence" }).textContent).toContain("Design notes");
    expect(screen.getByRole("button", { name: "Remove Design notes" })).toBeTruthy();
  });
});
