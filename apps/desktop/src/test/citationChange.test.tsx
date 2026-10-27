import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { EvidenceUsed } from "../web/common";
import { citationChange } from "../web/format";
import type { Citation } from "../types";

const cite = (overrides: Partial<Citation> = {}): Citation => ({
  artifact_id: "a1", chunk_id: "c1", title: "ledger.rs", path: "src/ledger.rs", start_line: 3, end_line: 9, confidence: 0.8,
  commit: null, last_author: null, last_changed_at: null, ...overrides,
});

afterEach(cleanup);

describe("citationChange", () => {
  it("names who last changed a repository file and when", () => {
    const day = new Date("2026-10-01T09:30:00Z").toLocaleDateString(undefined, { dateStyle: "medium" });
    expect(citationChange({ last_author: "Ada", last_changed_at: "2026-10-01T09:30:00Z" })).toBe(`changed by Ada on ${day}`);
    expect(citationChange({ last_author: " Ada ", last_changed_at: null })).toBe("changed by Ada");
    expect(citationChange({ last_author: null, last_changed_at: "2026-10-01T09:30:00Z" })).toBe(`changed on ${day}`);
  });

  it("says nothing for a citation that names neither, or a date it cannot read", () => {
    expect(citationChange({})).toBe("");
    expect(citationChange({ last_author: "", last_changed_at: "" })).toBe("");
    expect(citationChange({ last_author: null, last_changed_at: "not a date" })).toBe("");
  });
});

describe("evidence used by an answer", () => {
  it("shows the last author and date of a repository file beside its citation, and nothing for a note", () => {
    render(<EvidenceUsed citations={[cite({ last_author: "Ada", last_changed_at: "2026-10-01T09:30:00Z" }), cite({ artifact_id: "a2", chunk_id: "c2", title: "Runbook", path: "runbook.md" })]} onOpenArtifact={() => undefined} />);
    expect(screen.getAllByText(/changed by Ada/)).toHaveLength(1);
    expect(screen.getByRole("button", { name: /Runbook · runbook\.md/ })).toBeTruthy();
  });
});
