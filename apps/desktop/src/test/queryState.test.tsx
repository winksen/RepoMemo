import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { accountKeys, invalidateKeys, queryClient, workspaceInvalidation, workspaceKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";

const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;

beforeEach(() => { queryClient.clear(); });

describe("server state", () => {
  it("serves the fetched data and lets handlers update it like state", async () => {
    const fetchList = vi.fn().mockResolvedValue([1, 2]);
    const { result } = renderHook(() => useServerState<number[]>({ key: workspaceKeys.artifacts("w1"), fetch: fetchList, initial: [] }), { wrapper });
    expect(result.current[0]).toEqual([]);
    await waitFor(() => expect(result.current[0]).toEqual([1, 2]));
    act(() => result.current[1]((current) => [...current, 3]));
    await waitFor(() => expect(result.current[0]).toEqual([1, 2, 3]));
    expect(fetchList).toHaveBeenCalledTimes(1);
  });

  it("shows the cached value at once on a later visit, and refetches pages that ask to be always fresh", async () => {
    const fetchTasks = vi.fn().mockResolvedValueOnce(["old"]).mockResolvedValueOnce(["new"]);
    const options = { key: accountKeys.profileTasks("u1"), fetch: fetchTasks, initial: [] as string[], staleTime: 0 };
    const first = renderHook(() => useServerState<string[]>(options), { wrapper });
    await waitFor(() => expect(first.result.current[0]).toEqual(["old"]));
    first.unmount();
    const second = renderHook(() => useServerState<string[]>(options), { wrapper });
    expect(second.result.current[0]).toEqual(["old"]);
    await waitFor(() => expect(second.result.current[0]).toEqual(["new"]));
    expect(fetchTasks).toHaveBeenCalledTimes(2);
  });

  it("keeps a detail page's entry under its workspace, so a workspace reload refreshes it", async () => {
    const fetchCard = vi.fn().mockResolvedValue({ title: "card" });
    renderHook(() => useServerState<{ title: string } | null>({ key: workspaceKeys.memoryCard("w1", "c1"), fetch: fetchCard, initial: null }), { wrapper });
    await waitFor(() => expect(fetchCard).toHaveBeenCalledTimes(1));
    await queryClient.invalidateQueries({ queryKey: workspaceKeys.all("w1") });
    await waitFor(() => expect(fetchCard).toHaveBeenCalledTimes(2));
  });

  it("does not fetch while disabled", async () => {
    const fetchList = vi.fn().mockResolvedValue([]);
    renderHook(() => useServerState<number[]>({ key: workspaceKeys.tasks("w1"), fetch: fetchList, initial: [], enabled: false }), { wrapper });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(fetchList).not.toHaveBeenCalled();
  });

  it("refetches only what an event marked stale", async () => {
    const artifacts = vi.fn().mockResolvedValue(["a"]);
    const memory = vi.fn().mockResolvedValue(["m"]);
    const { result } = renderHook(() => ({
      artifacts: useServerState<string[]>({ key: workspaceKeys.artifacts("w1"), fetch: artifacts, initial: [] }),
      memory: useServerState<string[]>({ key: workspaceKeys.memoryCards("w1"), fetch: memory, initial: [] }),
    }), { wrapper });
    await waitFor(() => expect(result.current.memory[0]).toEqual(["m"]));
    artifacts.mockResolvedValue(["a", "b"]);
    await act(async () => { await invalidateKeys(workspaceInvalidation.ledger("w1")); });
    await waitFor(() => expect(result.current.artifacts[0]).toEqual(["a", "b"]));
    expect(artifacts).toHaveBeenCalledTimes(2);
    expect(memory).toHaveBeenCalledTimes(1);
  });
});
