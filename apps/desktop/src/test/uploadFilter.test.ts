import { describe, expect, it } from "vitest";
import { selectUploads } from "../lib/uploadFilter";

function file(name: string, size = 10, relativePath = "") {
  const value = new File([new Uint8Array(Math.min(size, 16))], name);
  Object.defineProperty(value, "size", { value: size });
  if (relativePath) Object.defineProperty(value, "webkitRelativePath", { value: relativePath });
  return value;
}

describe("selectUploads", () => {
  it("keeps supported files and counts the others", () => {
    const selection = selectUploads([
      file("notes.md"),
      file("Budget.XLSX"),
      file("movie.mp4"),
      file("huge.txt", 11 * 1024 * 1024),
      file("app.ts", 10, "repo/node_modules/dep/index.ts"),
      file("secret.txt", 10, "repo/.git/config"),
      file("src.rs", 10, "repo/src/lib.rs"),
    ], false);
    expect(selection.kept.map((entry) => entry.name)).toEqual(["notes.md", "Budget.XLSX", "src.rs"]);
    expect(selection).toMatchObject({ unsupported: 1, tooLarge: 1, ignored: 2 });
  });

  it("takes business documents only in the Documents view", () => {
    const selection = selectUploads([file("notes.md"), file("plan.docx"), file("deck.pptx")], true);
    expect(selection.kept.map((entry) => entry.name)).toEqual(["plan.docx", "deck.pptx"]);
    expect(selection.unsupported).toBe(1);
  });
});
