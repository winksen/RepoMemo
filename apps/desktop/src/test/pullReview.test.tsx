import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ getSharedArtifactMentions: vi.fn() }));

import { getSharedArtifactMentions } from "../lib/sharedApi";
import { ArtifactMentionsPanel } from "../web/Mentions";
import { describeReview, PullReviewSummary } from "../web/PullReview";

afterEach(() => cleanup());

describe("what the reviewers of a pull request said", () => {
  it("names who approved and who asked for changes, and nothing while nobody has decided", () => {
    expect(describeReview({ approved_by: ["ana", "cy"], changes_requested_by: ["bo"] })).toBe("Approved by ana, cy · Changes requested by bo");
    expect(describeReview({ approved_by: ["ana"], changes_requested_by: [] })).toBe("Approved by ana");
    expect(describeReview({ approved_by: [], changes_requested_by: [] })).toBeNull();
    expect(describeReview({})).toBeNull();
    expect(describeReview(undefined)).toBeNull();
    expect(describeReview({ approved_by: "ana" })).toBeNull();
  });

  it("shows the sentence as a status line, or nothing", () => {
    const { rerender } = render(<PullReviewSummary metadata={{ approved_by: ["ana"] }} />);
    expect(screen.getByRole("status").textContent).toBe("Approved by ana");
    rerender(<PullReviewSummary metadata={{ origin: "github_pull_request" }} />);
    expect(screen.queryByRole("status")).toBeNull();
  });
});

describe("the mentions of an issue or a pull request", () => {
  it("lists the items an item refers to, and the items that refer to it, with the number as it was written", async () => {
    vi.mocked(getSharedArtifactMentions).mockResolvedValue({
      mentioned_in: [{ artifact_id: "p13", title: "#13 Guard the ledger", artifact_type: "pr", path: "pulls/13", kind: "item", label: "#12" }],
      mentions: [{ artifact_id: "i4", title: "#4 Round half up", artifact_type: "issue", path: "issues/4", kind: "item", label: "acme/other#4" }],
    });
    render(<ArtifactMentionsPanel accessToken="t" artifactId="i12" onOpenArtifact={vi.fn()} />);
    await screen.findByText("#13 Guard the ledger");
    expect(screen.getByText(/refers to #12/)).toBeTruthy();
    expect(screen.getByText("#4 Round half up")).toBeTruthy();
    expect(screen.getByText(/refers to acme\/other#4/)).toBeTruthy();
  });
});
