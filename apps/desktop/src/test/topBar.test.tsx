import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", async (original) => ({ ...(await original<typeof import("../lib/sharedApi")>()), listSharedNotifications: vi.fn(), markSharedNotificationRead: vi.fn(), markAllSharedNotificationsRead: vi.fn() }));
vi.mock("../web/router", async (original) => ({ ...(await original<typeof import("../web/router")>()), navigate: vi.fn() }));

import { CairnButton } from "../components/CairnButton";
import { NotificationsMenu } from "../components/NotificationsMenu";
import { QuickAskDialog } from "../components/QuickAskDialog";
import { takeAssistantCommand } from "../lib/assistantBridge";
import { onQuickAskRequest } from "../lib/cairnBridge";
import { listSharedNotifications, markAllSharedNotificationsRead, markSharedNotificationRead } from "../lib/sharedApi";
import { navigate } from "../web/router";
import type { Organization, SharedNotification, SharedWorkspace } from "../types";

// Radix measures and scrolls; jsdom has neither.
beforeAll(() => {
  Object.assign(window.HTMLElement.prototype, { hasPointerCapture: () => false, releasePointerCapture: () => undefined, scrollIntoView: () => undefined, setPointerCapture: () => undefined });
  window.ResizeObserver = window.ResizeObserver ?? class { disconnect() { /* none */ } observe() { /* none */ } unobserve() { /* none */ } };
});
beforeEach(() => { vi.clearAllMocks(); window.localStorage.clear(); takeAssistantCommand(); });
afterEach(cleanup);

const notice = (id: string, title: string, read: boolean) => ({ id, workspace_id: "w1", notification_type: "task", title, body: `${title} body`, href: `/spaces/w1/tasks#${id}`, read_at: read ? "2026-11-20T10:00:00Z" : null, created_at: "2026-11-21T09:00:00Z" }) as SharedNotification;
const wrap = (node: React.ReactNode) => <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{node}</QueryClientProvider>;
const entry = (id: string, name: string) => ({ organization_id: "o1", role: "owner", workspace: { id, name, settings: {} } }) as unknown as SharedWorkspace;
const organizations = [{ id: "o1", name: "Bend", role: "owner" }] as Organization[];

it("lists the notifications in a dropdown from the bell, with the unread count, and opens one", async () => {
  vi.mocked(listSharedNotifications).mockResolvedValue([notice("n1", "Assigned: Retry policy", false), notice("n2", "Mentioned in Notes", true)]);
  vi.mocked(markSharedNotificationRead).mockResolvedValue(notice("n1", "Assigned: Retry policy", true));
  render(wrap(<NotificationsMenu accessToken="t" onNavigate={vi.fn()} userId="u1" />));
  const bell = await screen.findByRole("button", { name: "Notifications, 1 unread" });
  fireEvent.click(bell);
  expect(await screen.findByText("Assigned: Retry policy")).toBeTruthy();
  expect(screen.getByText("Mentioned in Notes")).toBeTruthy();
  expect(screen.getByRole("button", { name: "View all notifications" })).toBeTruthy();
});

it("opens what a notification points at and marks it read, and marks all read", async () => {
  const onNavigate = vi.fn();
  vi.mocked(listSharedNotifications).mockResolvedValue([notice("n1", "Assigned: Retry policy", false)]);
  vi.mocked(markSharedNotificationRead).mockResolvedValue(notice("n1", "Assigned: Retry policy", true));
  vi.mocked(markAllSharedNotificationsRead).mockResolvedValue(undefined as never);
  render(wrap(<NotificationsMenu accessToken="t" onNavigate={onNavigate} userId="u1" />));
  fireEvent.click(await screen.findByRole("button", { name: "Notifications, 1 unread" }));
  fireEvent.click(await screen.findByRole("button", { name: /Assigned: Retry policy/ }));
  await waitFor(() => expect(onNavigate).toHaveBeenCalledWith("/spaces/w1/tasks#n1"));
  expect(markSharedNotificationRead).toHaveBeenCalledWith("t", "n1");
});

it("takes the person to the full list from the dropdown", async () => {
  const onNavigate = vi.fn();
  vi.mocked(listSharedNotifications).mockResolvedValue([]);
  render(wrap(<NotificationsMenu accessToken="t" onNavigate={onNavigate} userId="u1" />));
  fireEvent.click(await screen.findByRole("button", { name: "Notifications" }));
  expect(await screen.findByText(/Nothing yet/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "View all notifications" }));
  expect(onNavigate).toHaveBeenCalledWith("/notifications");
});

it("opens CAIRN's page from its button, and offers the quick question from its menu", async () => {
  const onNavigate = vi.fn();
  const asked = vi.fn();
  const stop = onQuickAskRequest(asked);
  render(<CairnButton onNavigate={onNavigate} />);
  fireEvent.click(screen.getByRole("button", { name: "CAIRN" }));
  expect(onNavigate).toHaveBeenCalledWith("/cairn");
  fireEvent.keyDown(screen.getByRole("button", { name: "Quick ways to use CAIRN" }), { key: "ArrowDown" });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Quick question" }));
  expect(asked).toHaveBeenCalledTimes(1);
  stop();
});

it("asks a quick question: goes to a new chat in the chosen workspace and hands CAIRN the question", () => {
  const onClose = vi.fn();
  render(<QuickAskDialog onClose={onClose} open organizations={organizations} workspaces={[entry("w1", "Code"), entry("w2", "Pay")]} />);
  expect((screen.getByRole("button", { name: "Ask" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText(/Your question/), { target: { value: "  where are retries?  " } });
  fireEvent.click(screen.getByRole("button", { name: "Ask" }));
  expect(navigate).toHaveBeenCalledWith("/cairn");
  expect(takeAssistantCommand()).toEqual({ capability: null, message: "where are retries?" });
  expect(window.localStorage.getItem("repomemo.cairn.workspace")).toBe("w1");
  expect(onClose).toHaveBeenCalled();
});
