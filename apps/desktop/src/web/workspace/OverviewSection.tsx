import { SafeMarkdown } from "../../components/SafeMarkdown";
import { IconBrain as Brain, IconChartBar as Chart, IconCode as Code, IconLoader2 as Loader, IconRefresh as Refresh, IconShieldLock as Shield } from "@tabler/icons-react";
import { generateSharedWorkspaceAiOverview } from "../../lib/sharedApi";
import { type WorkspaceAiOverview } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { apiMessage, formatFileSize } from "../format";
import { EvidenceUsed, MetricBars, MetricTimeline } from "../common";
import { useCapabilities, useMetrics, useOverview } from "./data";
import { type WorkspacePage, sectionState } from "./page";
import { ICON_SIZE } from "../../components/ui/icon";

export function OverviewSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, canWrite, canInspectIndex, isLoading, isSubmitting, setIsSubmitting, setError, onOpenArtifact, draftMemoryFromAnswer } = page;
  const useS = sectionState(page.store, "overview");
  const [aiOverview, setAiOverview] = useS<WorkspaceAiOverview | null>("aiOverview", null);
  const [overview] = useOverview(accessToken, workspaceId);
  const [workspaceMetrics] = useMetrics(accessToken, workspaceId);
  const [capabilities] = useCapabilities(accessToken, workspaceId);
  async function generateAiOverview() {
    setIsSubmitting(true); setError(null);
    try { setAiOverview(await generateSharedWorkspaceAiOverview(accessToken, workspace.workspace.id)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
<>
        <><div className="shared-evidence-summary">
          <span><strong>{overview?.artifact_count ?? "—"}</strong> artifacts</span>{canInspectIndex ? <span><strong>{overview?.chunk_count ?? "—"}</strong> indexed chunks</span> : null}<span><strong>{overview?.memory_card_count ?? "—"}</strong> memory cards</span>
        </div>
        <section className="shared-metrics-dashboard" aria-labelledby="workspace-metrics-heading">
          <PanelHeading description={workspaceMetrics ? "Live API snapshot" : "Loading metrics…"} icon={Chart} id="workspace-metrics-heading" title="Workspace pulse" />
          <div className="shared-metric-highlights">
            <div><span>Index coverage</span><strong>{workspaceMetrics ? `${workspaceMetrics.artifact_count ? Math.round((workspaceMetrics.indexed_artifact_count / workspaceMetrics.artifact_count) * 100) : 0}%` : "—"}</strong><small>{workspaceMetrics ? `${workspaceMetrics.indexed_artifact_count} of ${workspaceMetrics.artifact_count} artifacts` : "Waiting for server data"}</small></div>
            <div><span>Indexed storage</span><strong>{workspaceMetrics ? formatFileSize(workspaceMetrics.indexed_artifact_bytes) : "—"}</strong><small>{workspaceMetrics ? `${formatFileSize(workspaceMetrics.pending_artifact_bytes)} pending` : "Waiting for server data"}</small></div>
            <div><span>Fresh evidence</span><strong>{workspaceMetrics?.artifacts_updated_last_7_days ?? "—"}</strong><small>{workspaceMetrics ? `${workspaceMetrics.artifacts_created_last_7_days} added in 7 days` : "Waiting for server data"}</small></div>
            <div><span>Knowledge density</span><strong>{workspaceMetrics ? `${workspaceMetrics.indexed_artifact_count ? Math.round(workspaceMetrics.chunk_count / workspaceMetrics.indexed_artifact_count) : 0}` : "—"}</strong><small>retrieval chunks per indexed file</small></div>
            <div><span>Code symbols</span><strong>{workspaceMetrics?.symbol_count ?? "—"}</strong><small>extracted from evidence</small></div>
            <div><span>Team access</span><strong>{workspaceMetrics?.member_count ?? "—"}</strong><small>people with workspace access</small></div>
            <div><span>Active tasks</span><strong>{workspaceMetrics ? workspaceMetrics.open_task_count + workspaceMetrics.in_progress_task_count : "—"}</strong><small>{workspaceMetrics?.in_progress_task_count ?? 0} currently in progress</small></div>
            <div><span>At risk</span><strong>{workspaceMetrics?.blocked_task_count ?? "—"}</strong><small>{workspaceMetrics?.overdue_task_count ?? 0} overdue action items</small></div>
            <div><span>Team discussion</span><strong>{workspaceMetrics?.comment_count ?? "—"}</strong><small>{workspaceMetrics?.completed_task_count ?? 0} tasks completed</small></div>
          </div>
          <div className="shared-metric-visuals">
            <section className="shared-metric-panel shared-metric-coverage"><div><h3>Indexing coverage</h3><span>{workspaceMetrics ? `${workspaceMetrics.pending_artifact_count} waiting to be indexed` : "Loading…"}</span></div><div className="shared-coverage-track" aria-label="Artifact indexing coverage" role="img"><span style={{ width: `${workspaceMetrics?.artifact_count ? (workspaceMetrics.indexed_artifact_count / workspaceMetrics.artifact_count) * 100 : 0}%` }} /></div><div className="shared-coverage-legend"><span>Indexed {workspaceMetrics?.indexed_artifact_count ?? 0}</span><span>Pending {workspaceMetrics?.pending_artifact_count ?? 0}</span></div></section>
            <MetricBars emptyLabel="Evidence types appear after you add files." items={workspaceMetrics?.artifact_types ?? []} title="Evidence by type" />
            <MetricBars emptyLabel="Storage distribution appears after you add files." formatValue={formatFileSize} items={workspaceMetrics?.artifact_bytes_by_type ?? []} title="Storage by evidence type" />
            <MetricBars emptyLabel="Languages appear after indexed files are detected." items={workspaceMetrics?.languages ?? []} title="Languages in evidence" />
            <MetricTimeline items={workspaceMetrics?.activity_by_day ?? []} />
            <MetricBars emptyLabel="Workspace changes will be summarized here." items={workspaceMetrics?.activity_actions ?? []} title="Recent activity" />
            <MetricBars emptyLabel="Team roles appear when members join this workspace." items={workspaceMetrics?.member_roles ?? []} summary={workspaceMetrics ? `${workspaceMetrics.member_count} people` : undefined} title="Team access" />
          </div>
        </section></>
        <section className="shared-ai-overview" aria-live="polite">
          <PanelHeading description={aiOverview?.provider_name ? aiOverview.provider_name : null} icon={Brain} title="AI workspace overview" />
          {aiOverview?.summary_markdown ? <div className="shared-markdown shared-ai-answer"><SafeMarkdown>{aiOverview.summary_markdown}</SafeMarkdown></div> : <p className="shared-muted-copy">Generate a concise project briefing from indexed evidence. RepoMemo only uses an enabled provider configured for this workspace and returns the source citations with the result.</p>}
          {aiOverview?.warnings.length ? <div className="shared-ai-overview-warning">{aiOverview.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}
          {aiOverview?.citations.length ? <EvidenceUsed accessToken={accessToken} citations={aiOverview.citations} onOpenArtifact={onOpenArtifact} /> : null}
          <div className="shared-ai-actions"><Button disabled={isSubmitting || isLoading || !capabilities?.can_generate_ai_overview} onClick={() => void generateAiOverview()} type="button" variant="secondary">{isSubmitting ? <Loader className="spin" size={ICON_SIZE.md} /> : <Brain size={ICON_SIZE.md} />}{aiOverview?.summary_markdown ? "Refresh AI overview" : "Generate AI overview"}</Button>{canWrite && aiOverview?.summary_markdown ? <Button disabled={isSubmitting} onClick={() => draftMemoryFromAnswer("Workspace overview", aiOverview.summary_markdown ?? "", "AI workspace overview", aiOverview.citations)} type="button" variant="secondary"><Shield size={ICON_SIZE.md} /> Save as memory</Button> : null}</div>
        </section>
</>
  );
}
