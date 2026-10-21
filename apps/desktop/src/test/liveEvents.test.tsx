import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useLiveEventHub } from "../lib/liveEvents";
import type { LiveEvent } from "../types";

function streamOf(chunks: string[]) {
  const encoder = new TextEncoder();
  return new Response(new ReadableStream({
    start(controller) {
      for (const chunk of chunks) controller.enqueue(encoder.encode(chunk));
      // Left open, like the server's stream.
    },
  }), { status: 200, headers: { "Content-Type": "text/event-stream" } });
}

afterEach(() => { vi.restoreAllMocks(); });

describe("live events", () => {
  it("delivers events split across chunks and ignores keep-alives and junk", async () => {
    const event = { type: "job", job: { id: "j1", status: "running", kind: "indexing" } };
    const text = JSON.stringify(event);
    // Subscribe before the stream answers, so nothing is missed.
    let answer: (response: Response) => void = () => undefined;
    vi.spyOn(globalThis, "fetch").mockReturnValue(new Promise<Response>((resolve) => { answer = resolve; }));
    const { result, unmount } = renderHook(() => useLiveEventHub("/v1/workspaces/w1/events", "token"));
    const received: LiveEvent[] = [];
    result.current.subscribe((next) => received.push(next));
    answer(streamOf([
      ": keep-alive\n\n",
      `event: job\ndata: ${text.slice(0, 20)}`,
      `${text.slice(20)}\n\n`,
      "data: not json\n\n",
    ]));
    await waitFor(() => expect(received.length).toBe(1));
    expect(received[0]).toEqual(event);
    unmount();
  });

  it("turns the changed notice a non-administrator gets into the activity event listeners know", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(streamOf([
      `event: changed\ndata: ${JSON.stringify({ type: "changed", action: "task_created", subject_type: "task", subject_id: "t1" })}\n\n`,
    ]));
    const { result, unmount } = renderHook(() => useLiveEventHub("/v1/workspaces/w1/events", "token"));
    const received: LiveEvent[] = [];
    result.current.subscribe((next) => received.push(next));
    await waitFor(() => expect(received.length).toBe(1));
    expect(received[0]).toEqual({
      type: "activity",
      event: { id: "", workspace_id: "", actor: null, action: "task_created", subject_type: "task", subject_id: "t1", summary: "", created_at: "" },
    });
    unmount();
  });

  it("opens nothing without a path", () => {
    const fetchMock = vi.spyOn(globalThis, "fetch");
    const { result } = renderHook(() => useLiveEventHub(null, "token"));
    expect(result.current.connected).toBe(false);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("live refresh", () => {
  it("reloads once after a burst of relevant events and ignores the rest", async () => {
    const { useLiveRefresh } = await import("../lib/liveEvents");
    const { LiveEventsContext } = await import("../lib/liveEvents");
    let listener: ((event: LiveEvent) => void) | null = null;
    const hub = {
      connected: true,
      subscribe: (next: (event: LiveEvent) => void) => {
        listener = next;
        return () => { listener = null; };
      },
    };
    const reload = vi.fn();
    const wrapper = ({ children }: { children: React.ReactNode }) => <LiveEventsContext.Provider value={hub}>{children}</LiveEventsContext.Provider>;
    renderHook(() => useLiveRefresh(reload, /^evidence_/, 20), { wrapper });
    const activity = (action: string) => ({ type: "activity", event: { id: "1", workspace_id: "w", actor: null, action, subject_type: "x", subject_id: null, summary: "", created_at: "" } }) as LiveEvent;
    listener!(activity("comment_created"));
    await new Promise((resolve) => setTimeout(resolve, 60));
    expect(reload).not.toHaveBeenCalled();
    listener!(activity("evidence_stored"));
    listener!(activity("evidence_uploaded"));
    listener!({ type: "job", job: { status: "completed" } } as unknown as LiveEvent);
    await waitFor(() => expect(reload).toHaveBeenCalledTimes(1));
  });
});
