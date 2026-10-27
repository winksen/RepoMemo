import { afterEach, describe, expect, it, vi } from "vitest";
import { searchSharedWorkspace } from "../lib/sharedApi";

afterEach(() => vi.restoreAllMocks());

describe("search budget response", () => {
  it("reports a timeout and forwards the cancellation signal", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("[]", {
      status: 200, headers: { "x-repomemo-query-state": "timed_out", "x-repomemo-semantic-search": "unavailable" },
    }));
    const controller = new AbortController();
    const result = await searchSharedWorkspace("token", "w1", { query: "broad", signal: controller.signal });
    expect(result).toEqual({ results: [], semanticAvailable: false, timedOut: true });
    expect(fetchMock.mock.calls[0][1]?.signal).toBe(controller.signal);
  });
});
