import { afterEach, describe, expect, it, vi } from "vitest";
import { listSystemJobsPage } from "../lib/sharedApi";

afterEach(() => vi.restoreAllMocks());

describe("System jobs cursor page", () => {
  it("keeps the status filter and reads the next cursor and total", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify([{ id: "job-2" }]), {
      status: 200, headers: { "X-Total-Count": "70", "X-Next-Cursor": "job-2" },
    }));
    const page = await listSystemJobsPage("token", "failed", "job-1", 24);
    const url = new URL(String(fetchMock.mock.calls[0][0]), "http://localhost");
    expect(url.searchParams.get("status")).toBe("failed");
    expect(url.searchParams.get("cursor")).toBe("job-1");
    expect(url.searchParams.get("limit")).toBe("24");
    expect([page.total, page.nextCursor, page.items.length]).toEqual([70, "job-2", 1]);
  });
});
