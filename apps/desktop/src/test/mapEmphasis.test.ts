import { describe, expect, it } from "vitest";

import { orderCoverage, orderLinks, relationRank } from "../web/map/emphasis";

const rows = ["Markdown", "PDF", "Code", "Repository", "Note", "Word"].map((label) => ({ label }));
const labels = (list: Array<{ label: string }>) => list.map((row) => row.label);

describe("the map's coverage rows", () => {
  it("keep their order without a preference", () => {
    expect(labels(orderCoverage(rows, undefined))).toEqual(labels(rows));
    expect(labels(orderCoverage(rows, []))).toEqual(labels(rows));
  });

  it("put the preferred kinds of file first, in the template's order, and the others after them as they were", () => {
    expect(labels(orderCoverage(rows, ["repository", "code"]))).toEqual(["Repository", "Code", "Markdown", "PDF", "Note", "Word"]);
    expect(labels(orderCoverage(rows, ["word", "pdf"]))).toEqual(["Word", "PDF", "Markdown", "Code", "Repository", "Note"]);
  });

  it("ignore a kind this client does not know or that the workspace has no files of, and do not change the rows", () => {
    expect(labels(orderCoverage(rows, ["spreadsheet", "email", "code"]))).toEqual(["Code", "Markdown", "PDF", "Repository", "Note", "Word"]);
    expect(rows).toHaveLength(6);
    expect(labels(rows)[0]).toBe("Markdown");
  });
});

describe("the map's links", () => {
  const links = [
    { kind: "cites", id: "c1" }, { kind: "similar", id: "s1" }, { kind: "cites", id: "c2" }, { kind: "similar", id: "s2" },
  ];
  const ids = (list: Array<{ id: string }>) => list.map((link) => link.id);

  it("rank similar files before memory citations unless the template prefers otherwise", () => {
    expect(relationRank("similar", undefined)).toBe(0);
    expect(relationRank("cites", undefined)).toBe(1);
    expect(relationRank("cites", ["cites"])).toBe(0);
    expect(relationRank("similar", ["cites"])).toBe(1);
    expect(relationRank("unknown", ["cites"])).toBe(2);
  });

  it("are listed with the preferred kind first, each kind keeping its order", () => {
    expect(ids(orderLinks(links, undefined))).toEqual(["s1", "s2", "c1", "c2"]);
    expect(ids(orderLinks(links, ["cites"]))).toEqual(["c1", "c2", "s1", "s2"]);
    expect(ids(orderLinks(links, ["cites", "similar"]))).toEqual(["c1", "c2", "s1", "s2"]);
  });

  it("are drawn with the preferred kind last, so it lies on top", () => {
    expect(ids(orderLinks(links, ["cites"], true))).toEqual(["s1", "s2", "c1", "c2"]);
    expect(ids(orderLinks(links, ["similar"], true))).toEqual(["c1", "c2", "s1", "s2"]);
    expect(links).toHaveLength(4);
  });
});
