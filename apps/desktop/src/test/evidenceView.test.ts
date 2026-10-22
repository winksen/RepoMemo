import { afterEach, describe, expect, it, vi } from "vitest";
import { categoryKind, DOCUMENT_KIND } from "../lib/evidenceView";
import { documentKindOf } from "../lib/documents";
import { fileCategory } from "../web/format";
import { FILE_CATEGORIES } from "../web/constants";
import { querySharedArtifactPage } from "../lib/sharedApi";
import type { ArtifactSummary } from "../types";

afterEach(() => vi.restoreAllMocks());

/** What the server's kind filter does with a file, written the way the SQL reads it. */
function matches(kind: ReturnType<typeof categoryKind>, file: Pick<ArtifactSummary, "artifact_type" | "path">) {
  const extension = file.path.split(".").pop()?.toLowerCase() ?? "";
  return (!kind.artifact_types?.length || kind.artifact_types.includes(file.artifact_type))
    && !(kind.not_artifact_types ?? []).includes(file.artifact_type)
    && (!kind.extensions?.length || kind.extensions.includes(extension))
    && !(kind.not_extensions ?? []).includes(extension);
}

describe("the evidence section's file types as server filters", () => {
  const files: Array<Pick<ArtifactSummary, "artifact_type" | "path">> = [
    { artifact_type: "file", path: "plan.docx" },
    { artifact_type: "file", path: "budget.XLSX" },
    { artifact_type: "file", path: "deck.pptx" },
    { artifact_type: "file", path: "manual.pdf" },
    { artifact_type: "file", path: "page.one" },
    { artifact_type: "file", path: "mail.eml" },
    { artifact_type: "file", path: "notes.txt" },
    { artifact_type: "markdown_doc", path: "guide.md" },
    { artifact_type: "note", path: "standup.note" },
    { artifact_type: "code_file", path: "main.rs" },
    { artifact_type: "image", path: "diagram.png" },
    { artifact_type: "repository", path: "/srv/repos/payments" },
    { artifact_type: "decision", path: "adr-1.md" },
  ];

  it("keeps every file in exactly the category the page gives it", () => {
    for (const file of files) {
      const own = fileCategory({ ...file, language: null });
      for (const { value } of FILE_CATEGORIES) {
        expect(matches(categoryKind(value), file), `${file.path} against ${value}`).toBe(value === own);
      }
    }
  });

  it("takes every document format, and only those, for the documents view", () => {
    for (const file of files) {
      expect(matches(DOCUMENT_KIND, file), file.path).toBe(documentKindOf(file) !== null);
    }
  });
});

describe("one page of a view of the evidence section", () => {
  const answer = (items: unknown[], total: number) => new Response(JSON.stringify(items), {
    status: 200,
    headers: { "Content-Type": "application/json", "X-Total-Count": String(total) },
  });

  it("asks for a folder, the top level or the whole ledger, and reads the total", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(answer([{ id: "a1" }], 61));
    const inFolder = await querySharedArtifactPage("token", "w1", { query: "", folderId: "f1", ledgerOnly: true, limit: 24, offset: 48 });
    const topLevel = await querySharedArtifactPage("token", "w1", { query: "", folderId: null, ledgerOnly: true, limit: 24, offset: 0 });
    const everywhere = await querySharedArtifactPage("token", "w1", { query: "plan", kinds: [categoryKind("word")], folderId: undefined, limit: 24, offset: 0 });
    const bodies = fetchMock.mock.calls.map(([, init]) => JSON.parse(String(init?.body)));
    expect([bodies[0].folder_id, bodies[0].top_level, bodies[0].ledger_only]).toEqual(["f1", false, true]);
    expect([bodies[1].folder_id, bodies[1].top_level]).toEqual([null, true]);
    expect([bodies[2].folder_id, bodies[2].top_level, bodies[2].query]).toEqual([null, false, "plan"]);
    expect(bodies[2].kinds[0].extensions).toEqual(["doc", "docx"]);
    const url = new URL(String(fetchMock.mock.calls[0][0]), "http://localhost");
    expect([url.searchParams.get("limit"), url.searchParams.get("offset")]).toEqual(["24", "48"]);
    expect(inFolder.total).toBe(61);
    expect(topLevel.items).toHaveLength(1);
    expect(everywhere.total).toBe(61);
  });
});
