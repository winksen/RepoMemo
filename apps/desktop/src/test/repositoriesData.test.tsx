import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ listSharedRepositories: vi.fn() }));
vi.mock("../lib/liveEvents", () => ({ useLiveEvents: vi.fn(() => false) }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { showToast } from "../components/ui/toast";
import { listSharedRepositories } from "../lib/sharedApi";
import { queryClient, workspaceKeys } from "../lib/queryClient";
import type { RepoSource } from "../types";
import { useRepositories } from "../web/repositories/useRepositories";

const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;

const repository = (overrides: Partial<RepoSource> = {}) => ({
  id: "r1", name: "atlas", status: "ready", active_job: null, last_error: null, last_report: null, ...overrides,
}) as unknown as RepoSource;
const running = { id: "j1", status: "running", kind: "repo_sync", source_id: "r1" };

beforeEach(() => { queryClient.clear(); vi.mocked(showToast).mockClear(); vi.mocked(listSharedRepositories).mockReset(); });

describe("repositories through the query cache", () => {
  it("loads the list into the cache and reads it again on demand", async () => {
    vi.mocked(listSharedRepositories).mockResolvedValueOnce({ repositories: [repository()] } as never).mockResolvedValueOnce({ repositories: [repository(), repository({ id: "r2", name: "orbit" })] } as never);
    const { result } = renderHook(() => useRepositories("token", "w1", vi.fn()), { wrapper });
    expect(result.current.repositories).toBeNull();
    await waitFor(() => expect(result.current.repositories).toHaveLength(1));
    await act(async () => { await result.current.load(); });
    expect(result.current.repositories).toHaveLength(2);
    expect(queryClient.getQueryData(workspaceKeys.repositories("w1"))).toHaveLength(2);
  });

  it("announces a sync once when it finishes, and tells the page", async () => {
    const onChanged = vi.fn();
    vi.mocked(listSharedRepositories)
      .mockResolvedValueOnce({ repositories: [repository({ active_job: running as never })] } as never)
      .mockResolvedValueOnce({ repositories: [repository({ last_report: { added: 2, restored: 0, updated: 1, renamed: 0, removed: 0 } as never })] } as never);
    const { result } = renderHook(() => useRepositories("token", "w1", onChanged), { wrapper });
    await waitFor(() => expect(result.current.repositories?.[0].active_job).toBeTruthy());
    expect(showToast).not.toHaveBeenCalled();
    await act(async () => { await result.current.load(); });
    await waitFor(() => expect(onChanged).toHaveBeenCalledTimes(1));
    expect(showToast).toHaveBeenCalledWith("success", expect.stringContaining("atlas synced."));
  });

  it("shows no toast for a list that arrives after the page was left", async () => {
    let answer: (value: unknown) => void = () => undefined;
    vi.mocked(listSharedRepositories).mockReturnValue(new Promise((resolve) => { answer = resolve; }) as never);
    const { unmount } = renderHook(() => useRepositories("token", "w1", vi.fn()), { wrapper });
    await waitFor(() => expect(listSharedRepositories).toHaveBeenCalledTimes(1));
    unmount();
    await act(async () => { answer({ repositories: [repository({ active_job: running as never })] }); await Promise.resolve(); });
    expect(showToast).not.toHaveBeenCalled();
  });
});
