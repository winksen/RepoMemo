import * as PopoverPrimitive from "@radix-ui/react-popover";
import { Bell } from "@phosphor-icons/react";
import { useState } from "react";
import { listSharedNotifications, markAllSharedNotificationsRead, markSharedNotificationRead } from "../lib/sharedApi";
import { accountKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import type { SharedNotification } from "../types";
import { formatRecentTime } from "../web/format";
import { Button } from "./ui/button";
import { ICON_SIZE } from "./ui/icon";
import { IconButton } from "./ui/icon-button";
import { showToast } from "./ui/toast";

const SHOWN = 8;

/**
 * The bell in the top bar: a dropdown with the latest notifications, not a page. A click opens what a notification points at and
 * marks it read; the full list is one link away. The number of unread ones shows on the bell and refreshes every minute.
 */
export function NotificationsMenu({ accessToken, onNavigate, userId }: { accessToken: string; onNavigate: (to: string) => void; userId: string }) {
  const [open, setOpen] = useState(false);
  const [notifications, setNotifications] = useServerState<SharedNotification[]>({
    key: accountKeys.notifications(userId), fetch: () => listSharedNotifications(accessToken), initial: [], staleTime: 0, refetchInterval: 60_000,
  });
  const unread = notifications.filter((notification) => !notification.read_at).length;

  async function openNotification(notification: SharedNotification) {
    setOpen(false);
    try {
      if (!notification.read_at) {
        const updated = await markSharedNotificationRead(accessToken, notification.id);
        setNotifications((current) => current.map((entry) => entry.id === updated.id ? updated : entry));
      }
    } catch { /* the link still opens; the notice stays unread */ }
    onNavigate(notification.href);
  }

  async function markAllRead() {
    try {
      await markAllSharedNotificationsRead(accessToken);
      setNotifications((current) => current.map((entry) => entry.read_at ? entry : { ...entry, read_at: new Date().toISOString() }));
    } catch { showToast("error", "The notifications could not be marked read."); }
  }

  return (
    <PopoverPrimitive.Root onOpenChange={setOpen} open={open}>
      <PopoverPrimitive.Trigger asChild>
        <IconButton className="shared-notifications-link" label={unread ? `Notifications, ${unread} unread` : "Notifications"} size="sm">
          <Bell size={ICON_SIZE.md} weight={unread ? "fill" : "regular"} />
          {unread ? <span aria-hidden="true" className="shared-notifications-count">{unread > 9 ? "9+" : unread}</span> : null}
        </IconButton>
      </PopoverPrimitive.Trigger>
      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content align="end" aria-label="Notifications" className="shared-notifications-popover" collisionPadding={12} sideOffset={8}>
          <div className="shared-notifications-popover-head">
            <strong>Notifications</strong>
            <Button disabled={!unread} onClick={() => void markAllRead()} type="button" variant="secondary">Mark all read</Button>
          </div>
          {notifications.length
            ? <ul className="shared-notifications-popover-list">{notifications.slice(0, SHOWN).map((notification) => <li key={notification.id}>
              <button className={notification.read_at ? "" : "is-unread"} onClick={() => void openNotification(notification)} type="button">
                <span className="shared-notifications-popover-dot" aria-hidden="true" />
                <span><strong>{notification.title}</strong><small>{notification.body}</small><time dateTime={notification.created_at}>{formatRecentTime(notification.created_at)}</time></span>
              </button>
            </li>)}</ul>
            : <p className="shared-notifications-popover-empty">Nothing yet. Task assignments and mentions appear here.</p>}
          <Button className="shared-notifications-popover-all" onClick={() => { setOpen(false); onNavigate("/notifications"); }} type="button" variant="secondary">View all notifications</Button>
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}
