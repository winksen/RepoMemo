import { useMemo } from "react";
import { type DashboardTemplate, type DashboardWorkspaceFigures, getSharedDashboard } from "../lib/sharedApi";
import { accountKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import type { SharedWorkspace } from "../types";
import { isPulseCard, PULSE_CARD_TITLES } from "./workspace/pulseCards";

/**
 * The figures of the workspaces listed, from the query cache: one request however many workspaces there are, shown at
 * once when the page was open before, refetched on every visit and again when the list changes.
 */
export function useDashboardFigures(accessToken: string, workspaces: SharedWorkspace[]) {
  const workspaceIds = useMemo(() => workspaces.map((entry) => entry.workspace.id).join(","), [workspaces]);
  return useServerState<Record<string, DashboardWorkspaceFigures>>({
    key: accountKeys.dashboard(workspaceIds),
    fetch: async () => Object.fromEntries((await getSharedDashboard(accessToken)).workspaces.map((entry) => [entry.workspace_id, entry])),
    initial: {},
    staleTime: 0,
  });
}

/** The template's headline figure in words, such as "1,204 code symbols"; nothing when the template names none or this client cannot draw it. */
export function headlineText(template: DashboardTemplate | null | undefined): string | null {
  const headline = template?.headline;
  if (!headline || !isPulseCard(headline.card)) return null;
  return `${headline.value.toLocaleString()} ${PULSE_CARD_TITLES[headline.card].toLowerCase()}`;
}

/** What a workspace's card says about its template: the name, and nothing for General. */
export function templateName(figures: DashboardWorkspaceFigures | undefined): string | null {
  return figures?.template?.name ?? null;
}

/** The figures on a workspace's card: files and active tasks as always, and the template's headline, which stands out. */
export function cardFigures(figures: DashboardWorkspaceFigures): Array<{ key: string; text: string; headline: boolean }> {
  const rows = [
    { key: "artifacts", text: `${figures.artifact_count} artifacts` },
    { key: "active_tasks", text: `${figures.active_task_count} active tasks` },
  ];
  const headline = figures.template?.headline ?? null;
  const named = headlineText(figures.template);
  if (headline && named && !rows.some((row) => row.key === headline.card)) rows.push({ key: headline.card, text: named });
  return rows.map((row) => ({ ...row, headline: row.key === headline?.card }));
}
