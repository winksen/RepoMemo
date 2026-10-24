import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { IMPORT_POLL_MS, importSharedWorkspaceArchive } from "../lib/sharedApi";
import { describeImportProgress } from "../components/WorkspaceImport";
import type { ArchiveImportJob, ArchiveImportProgress, ArchiveImportReport } from "../types";

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

const report = { dry_run: false, ok: true, workspace_name: "Notes", problems: [], warnings: [], workspace: null } as unknown as ArchiveImportReport;
const job = (overrides: Partial<ArchiveImportJob>): ArchiveImportJob => ({ id: "j1", status: "running", stage: "checking", progress_current: 0, progress_total: 0, report: null, error: null, ...overrides });
const archive = new Blob(["x"], { type: "application/gzip" });

beforeEach(() => { vi.useFakeTimers(); window.sessionStorage.clear(); });
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); cleanup(); });

/** Lets the polling timers run until the promise settles. */
async function settle<T>(promise: Promise<T>) {
  let outcome: { value?: T; error?: unknown } | null = null;
  promise.then((value) => { outcome = { value }; }, (error) => { outcome = { error }; });
  for (let turn = 0; turn < 40 && !outcome; turn += 1) await vi.advanceTimersByTimeAsync(IMPORT_POLL_MS);
  return outcome as { value?: T; error?: unknown } | null;
}

describe("importing a workspace archive", () => {
  it("sends the archive, follows the job through its stages and answers with its report", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(json({ job_id: "j1" }, 202))
      .mockResolvedValueOnce(json(job({ stage: "checking" })))
      .mockResolvedValueOnce(json(job({ stage: "storing_files", progress_current: 512, progress_total: 1024 })))
      .mockResolvedValueOnce(json(job({ stage: "finished", status: "done", progress_current: 1024, progress_total: 1024, report })));
    const seen: ArchiveImportProgress[] = [];
    const outcome = await settle(importSharedWorkspaceArchive("t", { organizationId: "o1", archive, dryRun: false, name: " Moved ", onProgress: (progress) => seen.push(progress) }));
    expect(outcome?.value).toEqual(report);
    // The first call sends the archive itself, for the job to be made; the others only ask.
    const [firstUrl, firstInit] = fetchMock.mock.calls[0];
    expect(String(firstUrl)).toContain("/v1/workspaces/import?");
    expect(String(firstUrl)).toContain("background=true");
    expect(String(firstUrl)).toContain("name=Moved");
    expect(firstInit?.method).toBe("POST");
    expect(firstInit?.body).toBe(archive);
    expect(String(fetchMock.mock.calls[1][0])).toContain("/v1/workspaces/import/jobs/j1");
    expect(seen.map((progress) => progress.stage)).toEqual(["uploading", "checking", "storing_files", "finished"]);
    expect(seen[2]).toEqual({ stage: "storing_files", current: 512, total: 1024 });
  });

  it("keeps a dry run a dry run", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(json({ job_id: "j2" }, 202))
      .mockResolvedValueOnce(json(job({ id: "j2", status: "done", stage: "finished", report })));
    await settle(importSharedWorkspaceArchive("t", { organizationId: "o1", archive, dryRun: true }));
    expect(String(fetchMock.mock.calls[0][0])).toContain("dry_run=true");
  });

  it("fails with the reason the server gives, or with what to do when the job is gone", async () => {
    vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(json({ job_id: "j3" }, 202))
      .mockResolvedValueOnce(json(job({ status: "failed", stage: "finished", error: "This file is not a readable workspace archive: it is empty." })));
    const failed = await settle(importSharedWorkspaceArchive("t", { organizationId: "o1", archive, dryRun: false }));
    expect(String((failed?.error as Error).message)).toContain("not a readable workspace archive");
    cleanup();

    vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(json({ job_id: "j4" }, 202))
      .mockResolvedValueOnce(json({ error: { message: "That import was not found." } }, 404));
    const lost = await settle(importSharedWorkspaceArchive("t", { organizationId: "o1", archive, dryRun: false }));
    expect(String((lost?.error as Error).message)).toContain("Nothing was created; try again");
  });

  it("still understands a server that answers the request with the result", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json(report, 201));
    const outcome = await settle(importSharedWorkspaceArchive("t", { organizationId: "o1", archive, dryRun: false }));
    expect(outcome?.value).toEqual(report);
  });

  it("is refused with the server's own message, such as a person who already has an import running", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(json({ error: { message: "An import of yours is still running." } }, 409));
    const outcome = await settle(importSharedWorkspaceArchive("t", { organizationId: "o1", archive, dryRun: false }));
    expect(String((outcome?.error as Error).message)).toContain("still running");
  });
});

describe("the progress of an import", () => {
  it("names the stage, and the share of the files stored while that is counted", () => {
    expect(describeImportProgress({ stage: "uploading", current: 0, total: 0 })).toBe("Sending the archive…");
    expect(describeImportProgress({ stage: "checking", current: 0, total: 0 })).toBe("Checking the archive…");
    expect(describeImportProgress({ stage: "storing_files", current: 512, total: 1024 })).toMatch(/^Storing the files: 50% of /);
    expect(describeImportProgress({ stage: "storing_files", current: 5000, total: 1024 })).toMatch(/: 100% of /);
    expect(describeImportProgress({ stage: "storing_files", current: 0, total: 0 })).toBe("Storing the files…");
    expect(describeImportProgress({ stage: "writing_records", current: 0, total: 0 })).toBe("Writing the workspace…");
  });
});
