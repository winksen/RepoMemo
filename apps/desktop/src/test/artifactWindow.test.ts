import { afterEach, describe, expect, it, vi } from "vitest";
import { ARTIFACT_WINDOW, listSharedArtifactWindow, querySharedArtifacts } from "../lib/sharedApi";

afterEach(() => vi.restoreAllMocks());

const answer = (items: unknown[], total: number) => new Response(JSON.stringify(items), {
  status: 200,
  headers: { "Content-Type": "application/json", "X-Total-Count": String(total) },
});

describe("the web client's file list", () => {
  it("asks for one page of the newest files and learns how many there are in all", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(answer([{ id: "a1" }, { id: "a2" }], 120_000));
    const window = await listSharedArtifactWindow("token", "w1");
    const url = new URL(String(fetchMock.mock.calls[0][0]), "http://localhost");
    expect(url.pathname).toBe("/v1/workspaces/w1/artifacts");
    expect([url.searchParams.get("limit"), url.searchParams.get("offset")]).toEqual([String(ARTIFACT_WINDOW), "0"]);
    expect(window.items.map((item) => item.id)).toEqual(["a1", "a2"]);
    expect(window.total).toBe(120_000);
  });

  it("never asks for the whole list: a search carries a limit the server accepts", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(answer([], 0));
    await querySharedArtifacts("token", "w1", { query: "retry", limit: 5_000 });
    await querySharedArtifacts("token", "w1", { query: "retry", limit: 12 });
    const limits = fetchMock.mock.calls.map(([url]) => new URL(String(url), "http://localhost").searchParams.get("limit"));
    expect(limits).toEqual([String(ARTIFACT_WINDOW), "12"]);
    expect(String(fetchMock.mock.calls[0][0])).toContain("/v1/workspaces/w1/artifacts/query?");
  });
});
