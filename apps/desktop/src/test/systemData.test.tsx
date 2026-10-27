import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { showToast } from "../components/ui/toast";
import { queryClient, systemKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import { useSystemData } from "../web/system/shared";

const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;

beforeEach(() => { queryClient.clear(); vi.mocked(showToast).mockClear(); });

describe("System pages and the query cache", () => {
  it("loads through the cache, shows the answer and fetches again on demand", async () => {
    const load = vi.fn().mockResolvedValueOnce({ users: 1 }).mockResolvedValueOnce({ users: 2 });
    const { result } = renderHook(() => useSystemData<{ users: number }>(systemKeys.overview(), load), { wrapper });
    expect(result.current[0]).toBeNull();
    await waitFor(() => expect(result.current[0]).toEqual({ users: 1 }));
    await act(async () => { await result.current[2](); });
    await waitFor(() => expect(result.current[0]).toEqual({ users: 2 }));
    expect(queryClient.getQueryData(systemKeys.overview())).toEqual({ users: 2 });
  });

  it("lets an answer that arrives after the page was left land in the cache only", async () => {
    let answer: (value: string[]) => void = () => undefined;
    const load = vi.fn().mockReturnValue(new Promise<string[]>((resolve) => { answer = resolve; }));
    const { unmount } = renderHook(() => useSystemData<string[]>(systemKeys.users(), load), { wrapper });
    await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
    unmount();
    await act(async () => { answer(["late"]); await Promise.resolve(); });
    expect(showToast).not.toHaveBeenCalled();
    await waitFor(() => expect(queryClient.getQueryData(systemKeys.users())).toEqual(["late"]));
  });

  it("shows the error as a toast with the page's own wording when the server gives no message", async () => {
    const load = vi.fn().mockRejectedValue(new Error(""));
    queryClient.setDefaultOptions({ queries: { retry: false } });
    renderHook(() => useSystemData<string[]>(systemKeys.audit(), load, { failure: "The audit trail could not be loaded." }), { wrapper });
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "The audit trail could not be loaded."));
    queryClient.setDefaultOptions({ queries: { retry: 1 } });
  });

  it("exposes inline errors without duplicating them as toasts", async () => {
    queryClient.setDefaultOptions({ queries: { retry: false } });
    const { result } = renderHook(() => useSystemData<string[]>(systemKeys.jobsAll(), () => Promise.reject(new Error("Jobs unavailable")), { silentError: true }), { wrapper });
    await waitFor(() => expect(result.current[4]?.message).toBe("Jobs unavailable"));
    expect(showToast).not.toHaveBeenCalled();
    queryClient.setDefaultOptions({ queries: { retry: 1 } });
  });

  it("keeps showing the last answer while a changed filter is fetched, when asked to", async () => {
    let finish: (value: string[]) => void = () => undefined;
    const load = vi.fn().mockResolvedValueOnce(["30 days"]).mockReturnValueOnce(new Promise<string[]>((resolve) => { finish = resolve; }));
    const { result, rerender } = renderHook(({ days }) => useSystemData<string[]>(systemKeys.usage(days), load, { keepPrevious: true }), { wrapper, initialProps: { days: 30 } });
    await waitFor(() => expect(result.current[0]).toEqual(["30 days"]));
    rerender({ days: 90 });
    expect(result.current[0]).toEqual(["30 days"]);
    await act(async () => { finish(["90 days"]); await Promise.resolve(); });
    await waitFor(() => expect(result.current[0]).toEqual(["90 days"]));
  });

  it("fetches again at the interval the data asks for, and stops when it says so", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const load = vi.fn().mockResolvedValueOnce({ running: true }).mockResolvedValueOnce({ running: true }).mockResolvedValue({ running: false });
      renderHook(() => useServerState<{ running: boolean } | null>({ key: systemKeys.backups(), fetch: load, initial: null, staleTime: 0, refetchInterval: (data) => (data?.running ? 2_000 : false) }), { wrapper });
      await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
      await act(async () => { await vi.advanceTimersByTimeAsync(2_100); });
      await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
      await act(async () => { await vi.advanceTimersByTimeAsync(2_100); });
      await waitFor(() => expect(load).toHaveBeenCalledTimes(3));
      await act(async () => { await vi.advanceTimersByTimeAsync(6_000); });
      expect(load).toHaveBeenCalledTimes(3);
    } finally {
      vi.useRealTimers();
    }
  });
});
