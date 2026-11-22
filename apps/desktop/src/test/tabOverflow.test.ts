import { describe, expect, it } from "vitest";
import { fitTabs } from "../web/tabOverflow";

const widths = new Map([["a", 100], ["b", 100], ["c", 100], ["d", 100]]);

describe("fitTabs", () => {
  it("keeps every tab when they all fit", () => {
    expect([...fitTabs(["a", "b", "c", "d"], widths, 400, 90, "a", false)]).toEqual(["a", "b", "c", "d"]);
  });

  it("leaves the tabs that do not fit behind More, in order, with room kept for it", () => {
    expect([...fitTabs(["a", "b", "c", "d"], widths, 300, 90, "a", false)]).toEqual(["a", "b"]);
  });

  it("always keeps the open tab, even a late one", () => {
    expect([...fitTabs(["a", "b", "c", "d"], widths, 300, 90, "d", false)]).toEqual(["d", "a"]);
  });

  it("reserves room for More when the template keeps sections in it", () => {
    expect([...fitTabs(["a", "b", "c", "d"], widths, 400, 90, "a", true)]).toEqual(["a", "b", "c"]);
  });
});
