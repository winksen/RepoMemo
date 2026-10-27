import { describe, expect, it } from "vitest";
import { answerExportMarkdown } from "../lib/answerExport";
import type { AskAnswer } from "../types";

const answer: AskAnswer = {
  answer_markdown: "A cited answer.",
  citations: [{ artifact_id: "artifact-1", chunk_id: "chunk-1", title: "Evidence", path: "/notes.md", start_line: 3, end_line: 5, confidence: null, commit: null, last_author: null, last_changed_at: null }],
  retrieved_context: [], confidence: null, warnings: [],
};

describe("answer export", () => {
  it("records a resolvable source and review state", async () => {
    const markdown = await answerExportMarkdown("Question?", answer, async () => ({ sourceId: "source-1", headingPath: "Overview", reviewStatus: "verified" }));
    expect(markdown).toContain("artifact `artifact-1`; source `source-1`; lines 3-5; review: verified");
  });

  it("marks a missing or inaccessible source explicitly", async () => {
    const markdown = await answerExportMarkdown("Question?", answer, async () => { throw new Error("gone"); });
    expect(markdown).toContain("MISSING SOURCE — artifact `artifact-1`, chunk `chunk-1`");
  });
});
