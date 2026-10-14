import { afterEach, describe, expect, it, vi } from "vitest";
import { indexSharedArtifact, waitForSharedJob } from "../lib/sharedApi";
import type { IndexingJobStatus } from "../types";

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

const job = (status: string): IndexingJobStatus => ({
  id: "j1", workspace_id: "w1", source_id: "s1", kind: "indexing", status, stage: "extracting_text",
  progress_current: status === "completed" ? 1 : 0, progress_total: 1, error_message: null, cancel_requested: false,
  created_at: "2026-10-14T00:00:00Z", updated_at: "2026-10-14T00:00:00Z",
});

afterEach(() => { vi.restoreAllMocks(); vi.useRealTimers(); });

describe("accepted jobs", () => {
  it("starts a re-index with a POST and gets the running job back", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(json(job("running"), 202));
    const started = await indexSharedArtifact("token", "a1");
    expect(started.status).toBe("running");
    const [url, init] = fetchMock.mock.calls[0];
    expect(String(url)).toContain("/v1/artifacts/a1/index");
    expect(init?.method).toBe("POST");
  });

  it("follows a job until it leaves running", async () => {
    vi.useFakeTimers();
    const fetchMock = vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(json(job("running")))
      .mockResolvedValueOnce(json(job("completed")));
    const waiting = waitForSharedJob("token", job("running"));
    await vi.advanceTimersByTimeAsync(2000);
    expect((await waiting).status).toBe("completed");
    expect(String(fetchMock.mock.calls[0][0])).toContain("/v1/jobs/j1");
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("returns a finished job without asking the server", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch");
    expect((await waitForSharedJob("token", job("failed"))).status).toBe("failed");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("stops following when the page that asked has gone", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch");
    const result = await waitForSharedJob("token", job("running"), () => true);
    expect(result.status).toBe("running");
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
