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

  it("opens nothing without a path", () => {
    const fetchMock = vi.spyOn(globalThis, "fetch");
    const { result } = renderHook(() => useLiveEventHub(null, "token"));
    expect(result.current.connected).toBe(false);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
