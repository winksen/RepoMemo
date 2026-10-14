import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { sectionState } from "../web/workspace/page";

describe("section state", () => {
  it("is there again when a section is shown again, and not shared with other sections", () => {
    const store = new Map<string, unknown>();
    const first = renderHook(() => sectionState(store, "evidence")<string>("query", ""));
    act(() => first.result.current[1]("adr"));
    expect(first.result.current[0]).toBe("adr");
    first.unmount();

    const again = renderHook(() => sectionState(store, "evidence")<string>("query", ""));
    expect(again.result.current[0]).toBe("adr");
    const other = renderHook(() => sectionState(store, "retrieval")<string>("query", ""));
    expect(other.result.current[0]).toBe("");
  });

  it("starts clean with a new store, as a different workspace does", () => {
    const store = new Map<string, unknown>([["evidence.query", "adr"]]);
    expect(renderHook(() => sectionState(store, "evidence")<string>("query", "")).result.current[0]).toBe("adr");
    const fresh = new Map<string, unknown>();
    expect(renderHook(() => sectionState(fresh, "evidence")<string>("query", "")).result.current[0]).toBe("");
  });
});
