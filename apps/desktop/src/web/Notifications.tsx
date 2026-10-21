import { useEffect, useState } from "react";
import { IconBell as Bell, IconChevronRight as ChevronRight } from "@tabler/icons-react";
import { listSharedNotifications, markAllSharedNotificationsRead, markSharedNotificationRead } from "../lib/sharedApi";
import { type Organization, type SharedNotification, type SharedSession, type SharedWorkspace } from "../types";
import { Button } from "../components/ui/button";
import { PageHeader } from "../components/ui/page-header";
import { EmptyState } from "../components/ui/empty-state";
import { Pagination } from "../components/ui/pagination";
import { usePagedList } from "../lib/usePagedList";
import { Toast, showToast } from "../components/ui/toast";
import { accountKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import { SharedLayout } from "../components/SharedLayout";
import { apiMessage, formatTime } from "./format";
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
  const paging = usePagedList(notifications);
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
      showToast("success", "All notifications are marked read.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  const unreadCount = notifications.filter((notification) => !notification.read_at).length;
  return <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizations={organizations} workspaces={workspaces} />}>
    <section className="shared-page-content shared-notifications-page" aria-busy={isLoading}>
      <PageHeader actions={<Button disabled={isMutating || unreadCount === 0} onClick={() => void markAllRead()} type="button" variant="secondary">Mark all read</Button>} description={<>Task assignments and direct evidence mentions appear here. Mention a teammate with their email, for example <code>@person@example.com</code>.</>} title="Notifications" />
      <Toast kind="error" message={error} />
      <section className="shared-notification-list" aria-label="Notifications">{notifications.length ? paging.rows.map((notification) => <article className={notification.read_at ? "" : "is-unread"} key={notification.id}><Button onClick={() => void openNotification(notification)} type="button" variant="secondary"><span className="shared-notification-icon"><Bell size={ICON_SIZE.md} /></span><span><strong>{notification.title}</strong><small>{notification.body}</small><time dateTime={notification.created_at}>{formatTime(notification.created_at)}</time></span><ChevronRight size={ICON_SIZE.md} /></Button></article>) : <EmptyState description="Assignments and evidence mentions will appear here when your team needs your attention." icon={<Bell size={25} />} title="Nothing new" />}</section>
      <Pagination {...paging.props} />
    </section>
  </SharedLayout>;
}
