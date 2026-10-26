import type { ReviewQueue, SharedNotification } from "../types";

/** What a look at the account's notices found. */
export type NoticeLook = {
  /** Unread notices not seen by an earlier look, oldest first: the ones to tell the person about. */
  fresh: SharedNotification[];
  /** Every notice id seen so far, to remember for the next look. */
  seen: Set<string>;
  unread: number;
};

/**
 * Compares the notices the server lists with those already seen. The first look (`seen` is `null`) tells about nothing: a person who
 * opens the app is not told about what was already waiting, which the badge shows.
 */
export function lookAtNotices(seen: Set<string> | null, notices: SharedNotification[]): NoticeLook {
  const now = new Set(notices.map((notice) => notice.id));
  const unread = notices.filter((notice) => !notice.read_at).length;
  if (seen === null) return { fresh: [], seen: now, unread };
  const fresh = notices
    .filter((notice) => !notice.read_at && !seen.has(notice.id))
    .sort((a, b) => a.created_at.localeCompare(b.created_at));
  return { fresh, seen: new Set([...seen, ...now]), unread };
}

/** What to say for the notices that arrived since the last look: itself when there is one, a count when there are several. */
export function wordNotices(fresh: SharedNotification[]): { title: string; body: string } | null {
  if (fresh.length === 0) return null;
  if (fresh.length === 1) return { title: fresh[0].title, body: fresh[0].body };
  return { title: `${fresh.length} new notices`, body: fresh.map((notice) => notice.title).slice(0, 3).join(" · ") };
}

/** The items waiting in the review queues of several workspaces (what the person snoozed is not waiting). */
export function reviewWaiting(queues: Array<Pick<ReviewQueue, "items">>): number {
  return queues.reduce((total, queue) => total + queue.items.length, 0);
}

/** Whether the window is out of sight or out of focus, which is when a system notification is wanted. */
export function windowIsInBackground(page: Pick<Document, "hasFocus" | "visibilityState"> = document): boolean {
  return page.visibilityState === "hidden" || !page.hasFocus();
}
