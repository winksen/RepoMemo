import { useEffect } from "react";
import { getSharedReviewQueue, listSharedNotifications } from "../lib/sharedApi";
import { currentNotifier } from "../lib/notifier";
import { lookAtNotices, reviewWaiting, windowIsInBackground, wordNotices } from "../lib/noticeWatch";

/** How often the account's notices are looked at, and how many of those looks also count the review queues. */
export const NOTICE_LOOK_MS = 60_000;
export const REVIEW_EVERY_LOOKS = 5;
/** The most workspaces whose review queues are counted. */
const MAX_QUEUES = 25;

/** Notices already seen, kept for the life of the page so a new access token does not tell about them again. */
const seenByUser = new Map<string, Set<string>>();

/**
 * Shows nothing. While the person is signed in it looks at their notices every minute, tells them of a new one with a system
 * notification when the window is in the background, and keeps the app's badge at the unread notices plus the items waiting in Review
 * (V0.4.53). The notices are the ones the person's own notification preferences let through. Without a notifier (a browser) it does
 * nothing.
 */
export function NoticeWatcher({ accessToken, userId, workspaceIds }: { accessToken: string; userId: string; workspaceIds: string[] }) {
  const workspaces = workspaceIds.slice(0, MAX_QUEUES).join(",");
  useEffect(() => {
    const notifier = currentNotifier();
    if (!notifier) return;
    let stopped = false;
    let looks = 0;
    let unread = 0;
    let waiting = 0;

    async function lookAgain() {
      const count = looks++;
      try {
        const found = lookAtNotices(seenByUser.get(userId) ?? null, await listSharedNotifications(accessToken));
        if (stopped) return;
        seenByUser.set(userId, found.seen);
        unread = found.unread;
        const message = wordNotices(found.fresh);
        if (message && windowIsInBackground()) await notifier!.notify(message).catch(() => false);
      } catch { /* the next look tries again */ }
      if (count % REVIEW_EVERY_LOOKS === 0) {
        const ids = workspaces ? workspaces.split(",") : [];
        const queues = await Promise.all(ids.map((id) => getSharedReviewQueue(accessToken, id).catch(() => null)));
        if (stopped) return;
        waiting = reviewWaiting(queues.filter((queue) => queue !== null));
      }
      if (!stopped) await notifier!.badge(unread + waiting).catch(() => undefined);
    }

    void lookAgain();
    const timer = window.setInterval(() => void lookAgain(), NOTICE_LOOK_MS);
    return () => {
      stopped = true;
      window.clearInterval(timer);
      void notifier.badge(0).catch(() => undefined);
    };
  }, [accessToken, userId, workspaces]);
  return null;
}
