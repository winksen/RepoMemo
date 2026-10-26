import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ listSharedNotifications: vi.fn(), getSharedReviewQueue: vi.fn() }));

import { setNotifier } from "../lib/notifier";
import { lookAtNotices, reviewWaiting, windowIsInBackground, wordNotices } from "../lib/noticeWatch";
import { getSharedReviewQueue, listSharedNotifications } from "../lib/sharedApi";
import { NOTICE_LOOK_MS, NoticeWatcher, REVIEW_EVERY_LOOKS } from "../web/NoticeWatcher";
import type { ReviewQueue, SharedNotification } from "../types";

const notice = (id: string, over: Partial<SharedNotification> = {}): SharedNotification => ({
  id, workspace_id: "w1", notification_type: "mention", title: `Notice ${id}`, body: `Body ${id}`, href: `/n/${id}`, read_at: null, created_at: `2026-10-26T10:0${id}:00Z`, ...over,
});
const queue = (items: number): ReviewQueue => ({ items: Array.from({ length: items }, (_, index) => ({ id: `i${index}` })) as unknown as ReviewQueue["items"], evidence_count: items, memory_card_count: 0, finding_count: 0, snoozed: [] });

describe("looking at the notices", () => {
  it("tells about nothing at the first look, only counts", () => {
    const first = lookAtNotices(null, [notice("1"), notice("2", { read_at: "2026-10-26T10:00:00Z" })]);
    expect(first.fresh).toEqual([]);
    expect(first.unread).toBe(1);
    expect([...first.seen].sort()).toEqual(["1", "2"]);
  });

  it("tells about unread notices not seen before, oldest first, and about no read one", () => {
    const seen = new Set(["1"]);
    const next = lookAtNotices(seen, [notice("3"), notice("1"), notice("2", { read_at: "2026-10-26T10:00:00Z" }), notice("4")]);
    expect(next.fresh.map((entry) => entry.id)).toEqual(["3", "4"]);
    expect(next.unread).toBe(3);
    expect([...next.seen].sort()).toEqual(["1", "2", "3", "4"]);
  });

  it("says one notice as itself and several as a count with the first titles", () => {
    expect(wordNotices([])).toBeNull();
    expect(wordNotices([notice("1")])).toEqual({ title: "Notice 1", body: "Body 1" });
    expect(wordNotices([notice("1"), notice("2"), notice("3"), notice("4")])).toEqual({ title: "4 new notices", body: "Notice 1 · Notice 2 · Notice 3" });
  });

  it("counts what waits in Review across workspaces, and knows when the window is out of sight", () => {
    expect(reviewWaiting([queue(2), queue(0), queue(3)])).toBe(5);
    expect(windowIsInBackground({ hasFocus: () => true, visibilityState: "visible" })).toBe(false);
    expect(windowIsInBackground({ hasFocus: () => false, visibilityState: "visible" })).toBe(true);
    expect(windowIsInBackground({ hasFocus: () => true, visibilityState: "hidden" })).toBe(true);
  });
});

describe("the watcher in the app's window", () => {
  const notifier = { notify: vi.fn(async (_notice: { title: string; body: string }) => true), badge: vi.fn(async (_count: number) => undefined) };
  let focused = false;

  beforeEach(() => {
    vi.useFakeTimers();
    notifier.notify.mockClear();
    notifier.badge.mockClear();
    vi.mocked(listSharedNotifications).mockReset();
    vi.mocked(getSharedReviewQueue).mockReset();
    vi.mocked(getSharedReviewQueue).mockResolvedValue(queue(2));
    focused = false;
    vi.spyOn(document, "hasFocus").mockImplementation(() => focused);
    setNotifier(notifier);
  });
  afterEach(() => { cleanup(); setNotifier(null); vi.useRealTimers(); vi.restoreAllMocks(); });

  const mount = (userId = "u1") => render(<NoticeWatcher accessToken="t" userId={userId} workspaceIds={["w1", "w2"]} />);
  const tick = (times = 1) => vi.advanceTimersByTimeAsync(NOTICE_LOOK_MS * times);

  it("marks the icon with the unread notices and what waits in Review, without telling about what was already there", async () => {
    vi.mocked(listSharedNotifications).mockResolvedValue([notice("1"), notice("2")]);
    mount("u-first");
    await vi.advanceTimersByTimeAsync(0);
    expect(notifier.notify).not.toHaveBeenCalled();
    expect(notifier.badge).toHaveBeenLastCalledWith(2 + 2 + 2);
    expect(getSharedReviewQueue).toHaveBeenCalledTimes(2);
  });

  it("tells about a new notice when the window is in the background, once", async () => {
    vi.mocked(listSharedNotifications).mockResolvedValueOnce([notice("1")]).mockResolvedValue([notice("1"), notice("2")]);
    mount("u-background");
    await vi.advanceTimersByTimeAsync(0);
    await tick();
    expect(notifier.notify).toHaveBeenCalledTimes(1);
    expect(notifier.notify).toHaveBeenCalledWith({ title: "Notice 2", body: "Body 2" });
    expect(notifier.badge).toHaveBeenLastCalledWith(2 + 2 + 2);
    await tick();
    expect(notifier.notify).toHaveBeenCalledTimes(1);
  });

  it("stays quiet while the window is in front, where the notices are on the page", async () => {
    focused = true;
    vi.mocked(listSharedNotifications).mockResolvedValueOnce([]).mockResolvedValue([notice("1")]);
    mount("u-front");
    await vi.advanceTimersByTimeAsync(0);
    await tick();
    expect(notifier.notify).not.toHaveBeenCalled();
    expect(notifier.badge).toHaveBeenLastCalledWith(1 + 4);
  });

  it("counts the review queues again only every few looks, and a failing one counts for nothing", async () => {
    vi.mocked(listSharedNotifications).mockResolvedValue([]);
    vi.mocked(getSharedReviewQueue).mockImplementation(async (_token, id) => { if (id === "w2") throw new Error("down"); return queue(3); });
    mount("u-queues");
    await vi.advanceTimersByTimeAsync(0);
    expect(getSharedReviewQueue).toHaveBeenCalledTimes(2);
    await tick(REVIEW_EVERY_LOOKS - 1);
    expect(getSharedReviewQueue).toHaveBeenCalledTimes(2);
    await tick();
    expect(getSharedReviewQueue).toHaveBeenCalledTimes(4);
    expect(notifier.badge).toHaveBeenLastCalledWith(3);
  });

  it("takes the badge away when the person signs out", async () => {
    vi.mocked(listSharedNotifications).mockResolvedValue([notice("1")]);
    const view = mount("u-out");
    await vi.advanceTimersByTimeAsync(0);
    view.unmount();
    expect(notifier.badge).toHaveBeenLastCalledWith(0);
    notifier.badge.mockClear();
    await tick(2);
    expect(notifier.badge).not.toHaveBeenCalled();
  });

  it("does nothing without a notifier, as in a browser", async () => {
    setNotifier(null);
    vi.mocked(listSharedNotifications).mockResolvedValue([notice("1")]);
    mount("u-browser");
    await tick(2);
    expect(listSharedNotifications).not.toHaveBeenCalled();
  });
});
