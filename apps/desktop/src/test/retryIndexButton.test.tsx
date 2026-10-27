import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ indexSharedArtifact: vi.fn(), waitForSharedJob: vi.fn() }));

import { indexSharedArtifact, waitForSharedJob } from "../lib/sharedApi";
import { RetryIndexButton } from "../components/RetryIndexButton";
import type { IndexingJobStatus } from "../types";

afterEach(() => { cleanup(); vi.clearAllMocks(); });

const accepted = { id: "job-12345678", status: "pending", error_message: null } as IndexingJobStatus;

describe("scoped index retry", () => {
  it("shows the accepted job and its terminal result", async () => {
    let finish!: (job: IndexingJobStatus) => void;
    vi.mocked(indexSharedArtifact).mockResolvedValue(accepted);
    vi.mocked(waitForSharedJob).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const onFinished = vi.fn();
    render(<RetryIndexButton accessToken="token" artifactId="artifact" onFinished={onFinished} />);
    fireEvent.click(screen.getByRole("button", { name: "Retry indexing" }));
    expect(await screen.findByText("Index job job-1234: pending")).toBeTruthy();
    finish({ ...accepted, status: "completed" });
    expect(await screen.findByText("Index job job-1234: completed")).toBeTruthy();
    expect(onFinished).toHaveBeenCalledOnce();
    expect(indexSharedArtifact).toHaveBeenCalledWith("token", "artifact");
  });

  it("shows the server's refusal reason", async () => {
    vi.mocked(indexSharedArtifact).mockRejectedValue(new Error("This item cannot be indexed."));
    render(<RetryIndexButton accessToken="token" artifactId="artifact" onFinished={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Retry indexing" }));
    expect((await screen.findByRole("alert")).textContent).toBe("This item cannot be indexed.");
  });
});
