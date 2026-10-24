import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearRecents, MAX_RECENTS, readRecents, rememberRecent, type RecentItem } from "../lib/paletteRecents";

const item = (id: string, overrides: Partial<RecentItem> = {}): RecentItem => ({ id, label: `Item ${id}`, hint: "Atlas", to: `/workspaces/w1/artifacts/${id}`, workspaceId: "w1", ...overrides });

beforeEach(() => window.localStorage.clear());
afterEach(() => window.localStorage.clear());

describe("recent places of the command palette", () => {
  it("keeps the newest first, each place once, and only the newest few", () => {
    for (const id of ["a", "b", "c"]) rememberRecent("u1", item(id));
    rememberRecent("u1", item("a", { label: "Renamed" }));
    expect(readRecents("u1").map((entry) => entry.id)).toEqual(["a", "c", "b"]);
    expect(readRecents("u1")[0].label).toBe("Renamed");
    for (let index = 0; index < MAX_RECENTS + 4; index += 1) rememberRecent("u1", item(`n${index}`));
    const kept = readRecents("u1");
    expect(kept).toHaveLength(MAX_RECENTS);
    expect(kept[0].id).toBe(`n${MAX_RECENTS + 3}`);
  });

  it("survives a reload: it is read back from the browser's storage", () => {
    rememberRecent("u1", item("a"));
    expect(JSON.parse(window.localStorage.getItem("repomemo.palette.recent.u1") ?? "[]")).toHaveLength(1);
    expect(readRecents("u1")[0]).toEqual(item("a"));
  });

  it("is one person's: another account on the same browser sees none of it", () => {
    rememberRecent("u1", item("a"));
    expect(readRecents("u2")).toEqual([]);
    expect(readRecents("")).toEqual([]);
    rememberRecent("", item("b"));
    expect(window.localStorage.length).toBe(1);
    clearRecents("u1");
    expect(readRecents("u1")).toEqual([]);
  });

  it("refuses an address that is not a path of the app, however it got there", () => {
    for (const to of ["https://evil.example/x", "//evil.example/x", "javascript:alert(1)", "workspaces/w1", "/workspaces/w1?x=<script>"]) {
      rememberRecent("u1", item("bad", { to }));
    }
    expect(readRecents("u1")).toEqual([]);
    window.localStorage.setItem("repomemo.palette.recent.u1", JSON.stringify([item("ok"), { id: "x", label: "Tampered", to: "https://evil.example", workspaceId: "w1" }, "nonsense", null, item("also-ok")]));
    expect(readRecents("u1").map((entry) => entry.id)).toEqual(["ok", "also-ok"]);
  });

  it("reads nothing from damaged storage", () => {
    window.localStorage.setItem("repomemo.palette.recent.u1", "{not json");
    expect(readRecents("u1")).toEqual([]);
    window.localStorage.setItem("repomemo.palette.recent.u1", JSON.stringify({ not: "a list" }));
    expect(readRecents("u1")).toEqual([]);
  });
});
