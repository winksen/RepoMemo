import { useEffect } from "react";
import { IconChevronRight as ChevronRight, IconChecklist as Checklist } from "@tabler/icons-react";
import { getSharedMyWork } from "../lib/sharedApi";
import { accountKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import type { MyWork } from "../types";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage, formatTaskDueDate } from "../web/format";
import { t } from "../lib/messages";
import { showToast } from "./ui/toast";
import { navigate } from "../web/router";

const EMPTY: MyWork = { tasks: [], reviews: [], mentions: [], overdue_task_count: 0, total_count: 0 };

/**
 * What waits for the signed-in person across the workspaces they can open (V0.6.2): assigned tasks, files they own that
 * need review and unread mentions, in one list. The server filters by current access, so work in a workspace the
 * person lost is neither shown nor counted.
 */
export function MyWorkPanel({ accessToken, userId }: { accessToken: string; userId: string }) {
  const [work, , query] = useServerState<MyWork>({
    key: accountKeys.myWork(userId), fetch: () => getSharedMyWork(accessToken), initial: EMPTY, staleTime: 0,
  });
  // A failed read is said, not shown as an empty list: "nothing is waiting" would be a claim the page cannot back up.
  useEffect(() => { if (query.isError) showToast("error", apiMessage(query.error, t("myWork.readFailed"))); }, [query.isError, query.error]);
  if (query.isLoading || (query.isError && work.total_count === 0)) return null;
  return (
    <section aria-labelledby="my-work-heading" className="rm-my-work">
      <PanelHeading description={work.total_count ? (work.overdue_task_count ? t("myWork.waitingOverdue", { total: work.total_count, overdue: work.overdue_task_count }) : t("myWork.waiting", { total: work.total_count })) : null} icon={Checklist} id="my-work-heading" title={t("myWork.title")} />
      {work.total_count === 0 ? <p className="shared-muted-copy">{t("myWork.empty")}</p> : (
        <ul className="rm-my-work-list">
          {work.tasks.map((task) => <li key={`task-${task.id}`}><Button className="rm-my-work-item" onClick={() => navigate(`/workspaces/${encodeURIComponent(task.workspace_id)}/tasks`)} type="button" variant="secondary"><span><strong>{task.title}</strong><small>{task.due_at ? t("myWork.taskDue", { priority: task.priority, due: formatTaskDueDate(task.due_at) }) : t("myWork.task", { priority: task.priority })}</small></span><ChevronRight size={ICON_SIZE.md} /></Button></li>)}
          {work.reviews.map((review) => <li key={`review-${review.artifact_id}`}><Button className="rm-my-work-item" onClick={() => navigate(`/workspaces/${encodeURIComponent(review.workspace_id)}/review`)} type="button" variant="secondary"><span><strong>{review.title}</strong><small>{review.reason ? t("myWork.reviewReason", { workspace: review.workspace_name, reason: review.reason }) : t("myWork.review", { workspace: review.workspace_name })}</small></span><ChevronRight size={ICON_SIZE.md} /></Button></li>)}
          {work.mentions.map((mention) => <li key={`mention-${mention.id}`}><Button className="rm-my-work-item" onClick={() => navigate(mention.href)} type="button" variant="secondary"><span><strong>{mention.title}</strong><small>{mention.body ? t("myWork.mentionBody", { body: mention.body }) : t("myWork.mention")}</small></span><ChevronRight size={ICON_SIZE.md} /></Button></li>)}
        </ul>
      )}
    </section>
  );
}
