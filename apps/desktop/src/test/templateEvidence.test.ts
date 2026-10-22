import { describe, expect, it } from "vitest";

import { FILE_CATEGORIES } from "../web/constants";
import { defaultLedgerView, orderedFileCategories } from "../web/templateEvidence";

const values = (categories: typeof FILE_CATEGORIES) => categories.map((category) => category.value);

describe("the evidence ledger's defaults from a template", () => {
  it("opens as a grid without a template or when the template asks for one, and as a list when it asks", () => {
    expect(defaultLedgerView(null)).toBe("grid");
    expect(defaultLedgerView(undefined)).toBe("grid");
    expect(defaultLedgerView({ default_view: "grid" })).toBe("grid");
    expect(defaultLedgerView({ default_view: "list" })).toBe("list");
    expect(defaultLedgerView({ default_view: "documents" })).toBe("grid");
  });

  it("lists the file types in the usual order without a preference", () => {
    expect(values(orderedFileCategories(null))).toEqual(values(FILE_CATEGORIES));
    expect(values(orderedFileCategories({ types_first: [] }))).toEqual(values(FILE_CATEGORIES));
  });

  it("puts the preferred file types first, in the template's order, and keeps the others after them in the usual order", () => {
    const ordered = values(orderedFileCategories({ types_first: ["pdf", "word", "excel", "powerpoint"] }));
    expect(ordered.slice(0, 4)).toEqual(["pdf", "word", "excel", "powerpoint"]);
    expect(ordered.slice(4)).toEqual(["markdown", "note", "text", "code", "onenote", "email", "image", "repository"]);
    expect(ordered).toHaveLength(FILE_CATEGORIES.length);
  });

  it("ignores a type this client does not list and a type named twice", () => {
    const ordered = values(orderedFileCategories({ types_first: ["spreadsheet", "code", "code"] }));
    expect(ordered[0]).toBe("code");
    expect(ordered.filter((value) => value === "code")).toHaveLength(1);
    expect(ordered).toHaveLength(FILE_CATEGORIES.length);
  });
});
