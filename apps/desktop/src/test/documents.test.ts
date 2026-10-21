import { describe, expect, it } from "vitest";
import { canOpenInApp, hasMacros } from "../lib/documents";

describe("Open in Office", () => {
  it("is offered for Office files without macros", () => {
    for (const path of ["report.docx", "Budget.XLSX", "deck.pptx", "notes.pdf", "old.doc"]) expect(canOpenInApp({ path }), path).toBe(true);
  });

  it("is not offered for formats that can carry macros, nor for unknown files", () => {
    for (const path of ["macros.xlsm", "Template.DOCM", "show.pptm", "book.xlsb", "readme.md", "noextension"]) expect(canOpenInApp({ path }), path).toBe(false);
    expect(hasMacros({ path: "macros.xlsm" })).toBe(true);
    expect(hasMacros({ path: "report.docx" })).toBe(false);
  });
});
