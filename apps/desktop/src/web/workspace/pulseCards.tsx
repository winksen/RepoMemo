import type { ReactNode } from "react";
import type { WorkspaceMetrics, WorkspaceOverview } from "../../types";
import { formatBytes } from "../format";

/** The figures the Overview can show as a card, by id: what a template's `overview.cards` may name (V0.2.174). */
export const PULSE_CARD_IDS = ["index_coverage", "indexed_storage", "fresh_evidence", "knowledge_density", "code_symbols", "sources", "memory_cards", "team_access", "active_tasks", "at_risk", "team_discussion"] as const;
export type PulseCardId = typeof PULSE_CARD_IDS[number];

/** The nine cards a workspace has always shown, in their order; General and a template without an `overview` block use them. */
export const DEFAULT_PULSE_CARDS: PulseCardId[] = ["index_coverage", "indexed_storage", "fresh_evidence", "knowledge_density", "code_symbols", "team_access", "active_tasks", "at_risk", "team_discussion"];

/** What a card is called, for a person choosing between templates. */
export const PULSE_CARD_TITLES: Record<PulseCardId, string> = {
  index_coverage: "Index coverage", indexed_storage: "Indexed storage", fresh_evidence: "Fresh evidence", knowledge_density: "Knowledge density",
  code_symbols: "Code symbols", sources: "Sources", memory_cards: "Memory cards", team_access: "Team access", active_tasks: "Active tasks",
  at_risk: "At risk", team_discussion: "Team discussion",
};

export const isPulseCard = (value: string): value is PulseCardId => (PULSE_CARD_IDS as readonly string[]).includes(value);

/** The ids to draw: the template's list without any this client cannot draw; the usual nine if nothing is left. */
export function pulseCardIds(requested: string[] | null | undefined): PulseCardId[] {
  const known = (requested ?? []).filter(isPulseCard).filter((id, index, all) => all.indexOf(id) === index);
  return known.length ? known : DEFAULT_PULSE_CARDS;
}

const WAITING = "Waiting for server data";

/** One card of the Workspace pulse, drawn from the metrics and overview the page has already loaded; nothing is asked of the server. */
export function pulseCard(id: PulseCardId, metrics: WorkspaceMetrics | null, overview: WorkspaceOverview | null): ReactNode {
  switch (id) {
    case "index_coverage": return <div key={id}><span>Index coverage</span><strong>{metrics ? `${metrics.artifact_count ? Math.round((metrics.indexed_artifact_count / metrics.artifact_count) * 100) : 0}%` : "—"}</strong><small>{metrics ? `${metrics.indexed_artifact_count} of ${metrics.artifact_count} artifacts` : WAITING}</small></div>;
    case "indexed_storage": return <div key={id}><span>Indexed storage</span><strong>{metrics ? formatBytes(metrics.indexed_artifact_bytes) : "—"}</strong><small>{metrics ? `${formatBytes(metrics.pending_artifact_bytes)} pending` : WAITING}</small></div>;
    case "fresh_evidence": return <div key={id}><span>Fresh evidence</span><strong>{metrics?.artifacts_updated_last_7_days ?? "—"}</strong><small>{metrics ? `${metrics.artifacts_created_last_7_days} added in 7 days` : WAITING}</small></div>;
    case "knowledge_density": return <div key={id}><span>Knowledge density</span><strong>{metrics ? `${metrics.indexed_artifact_count ? Math.round(metrics.chunk_count / metrics.indexed_artifact_count) : 0}` : "—"}</strong><small>retrieval chunks per indexed file</small></div>;
    case "code_symbols": return <div key={id}><span>Code symbols</span><strong>{metrics?.symbol_count ?? "—"}</strong><small>extracted from evidence</small></div>;
    case "sources": return <div key={id}><span>Sources</span><strong>{metrics?.source_count ?? "—"}</strong><small>uploads and linked repositories</small></div>;
    case "memory_cards": return <div key={id}><span>Memory cards</span><strong>{overview?.memory_card_count ?? "—"}</strong><small>team knowledge kept</small></div>;
    case "team_access": return <div key={id}><span>Team access</span><strong>{metrics?.member_count ?? "—"}</strong><small>people with workspace access</small></div>;
    case "active_tasks": return <div key={id}><span>Active tasks</span><strong>{metrics ? metrics.open_task_count + metrics.in_progress_task_count : "—"}</strong><small>{metrics?.in_progress_task_count ?? 0} currently in progress</small></div>;
    case "at_risk": return <div key={id}><span>At risk</span><strong>{metrics?.blocked_task_count ?? "—"}</strong><small>{metrics?.overdue_task_count ?? 0} overdue action items</small></div>;
    case "team_discussion": return <div key={id}><span>Team discussion</span><strong>{metrics?.comment_count ?? "—"}</strong><small>{metrics?.completed_task_count ?? 0} tasks completed</small></div>;
  }
}
