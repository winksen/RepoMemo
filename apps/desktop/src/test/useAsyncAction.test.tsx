import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { showToast } from "../components/ui/toast";
import { useAsyncAction } from "../lib/useAsyncAction";

beforeEach(() => { vi.mocked(showToast).mockClear(); });

describe("useAsyncAction", () => {
  it("is busy while the action runs and shows the success toast when it finishes", async () => {
    let finish: () => void = () => undefined;
    const { result } = renderHook(() => useAsyncAction());
    expect(result.current.isBusy).toBe(false);
    let outcome: Promise<boolean> = Promise.resolve(false);
    act(() => { outcome = result.current.run(() => new Promise<void>((resolve) => { finish = resolve; }), { success: "Saved." }); });
    await waitFor(() => expect(result.current.isBusy).toBe(true));
    expect(showToast).not.toHaveBeenCalled();
    await act(async () => { finish(); await outcome; });
    expect(await outcome).toBe(true);
    expect(result.current.isBusy).toBe(false);
    expect(showToast).toHaveBeenCalledWith("success", "Saved.");
  });

  it("builds the success message from what the action returned", async () => {
    const { result } = renderHook(() => useAsyncAction());
    await act(async () => { await result.current.run(async () => ({ sent: true }), { success: (answer) => (answer.sent ? "Sent." : "Not sent.") }); });
    expect(showToast).toHaveBeenCalledWith("success", "Sent.");
  });

  it("shows the error's message, or the fallback when it has none, and reports the failure", async () => {
    const { result } = renderHook(() => useAsyncAction());
    let first = true;
    await act(async () => { first = await result.current.run(async () => { throw new Error("The name is taken."); }, { failure: "Could not save." }); });
    expect(first).toBe(false);
    expect(showToast).toHaveBeenLastCalledWith("error", "The name is taken.");
    await act(async () => { await result.current.run(async () => { throw new Error(""); }, { failure: "Could not save." }); });
    expect(showToast).toHaveBeenLastCalledWith("error", "Could not save.");
    expect(result.current.isBusy).toBe(false);
  });

  it("stays busy until the last of several overlapping actions is done", async () => {
    const finishers: Array<() => void> = [];
    const { result } = renderHook(() => useAsyncAction());
    const pause = () => new Promise<void>((resolve) => { finishers.push(resolve); });
    const runs: Array<Promise<boolean>> = [];
    act(() => { runs.push(result.current.run(pause), result.current.run(pause)); });
    await act(async () => { finishers[0](); await runs[0]; });
    expect(result.current.isBusy).toBe(true);
    await act(async () => { finishers[1](); await runs[1]; });
    expect(result.current.isBusy).toBe(false);
  });
});
