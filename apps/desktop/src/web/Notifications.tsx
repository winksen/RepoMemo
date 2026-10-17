import { useEffect, useState } from "react";
import { IconBell as Bell, IconChevronRight as ChevronRight } from "@tabler/icons-react";
import { listSharedNotifications, markAllSharedNotificationsRead, markSharedNotificationRead } from "../lib/sharedApi";
import { type Organization, type SharedNotification, type SharedSession, type SharedWorkspace } from "../types";
import { Button } from "../components/ui/button";
import { Toast } from "../components/ui/toast";
import { accountKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import { SharedLayout } from "../components/SharedLayout";
import { apiMessage, formatActivityTime } from "./format";
import { navigate } from "./router";
import { OrganizationRail } from "./Organization";
import { ICON_SIZE } from "../components/ui/icon";

export function SharedNotifications({
  accessToken,
  apiAvailable,
  organizations,
  session,
  signOut,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  const [notifications, setNotifications, notificationsQuery] = useServerState<SharedNotification[]>({
    key: accountKeys.notifications(session.user.id), fetch: () => listSharedNotifications(accessToken), initial: [], staleTime: 0,
  });
  const [error, setError] = useState<string | null>(null);
  const isLoading = notificationsQuery.isLoading;
  const [isMutating, setIsMutating] = useState(false);
  useEffect(() => { if (notificationsQuery.error) setError(apiMessage(notificationsQuery.error)); }, [notificationsQuery.error]);

  async function openNotification(notification: SharedNotification) {
    setError(null);
    try {
      if (!notification.read_at) {
        const updated = await markSharedNotificationRead(accessToken, notification.id);
        setNotifications((current) => current.map((entry) => entry.id === updated.id ? updated : entry));
      }
      navigate(notification.href);
    } catch (requestError) { setError(apiMessage(requestError)); }
  }

  async function markAllRead() {
    setIsMutating(true); setError(null);
    try {
      await markAllSharedNotificationsRead(accessToken);
      setNotifications((current) => current.map((entry) => entry.read_at ? entry : { ...entry, read_at: new Date().toISOString() }));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  const unreadCount = notifications.filter((notification) => !notification.read_at).length;
  return <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizations={organizations} workspaces={workspaces} />}>
    <section className="shared-page-content shared-notifications-page" aria-busy={isLoading}>
      <div className="shared-detail-heading"><div><h1>Notifications</h1><p>Task assignments and direct evidence mentions appear here. Mention a teammate with their email, for example <code>@person@example.com</code>.</p></div><div className="shared-detail-actions"><Button disabled={isMutating || unreadCount === 0} onClick={() => void markAllRead()} type="button" variant="secondary">Mark all read</Button></div></div>
      <Toast kind="error" message={error} />
      <section className="shared-notification-list" aria-label="Notifications">{notifications.length ? notifications.map((notification) => <article className={notification.read_at ? "" : "is-unread"} key={notification.id}><Button onClick={() => void openNotification(notification)} type="button" variant="secondary"><span className="shared-notification-icon"><Bell size={ICON_SIZE.md} /></span><span><strong>{notification.title}</strong><small>{notification.body}</small><time dateTime={notification.created_at}>{formatActivityTime(notification.created_at)}</time></span><ChevronRight size={ICON_SIZE.md} /></Button></article>) : <div className="shared-empty-state"><Bell size={25} /><strong>Nothing new</strong><span>Assignments and evidence mentions will appear here when your team needs your attention.</span></div>}</section>
    </section>
  </SharedLayout>;
}
