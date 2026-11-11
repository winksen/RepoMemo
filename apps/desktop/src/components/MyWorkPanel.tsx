import { IconChevronRight as ChevronRight, IconChecklist as Checklist } from "@tabler/icons-react";
import { getSharedMyWork } from "../lib/sharedApi";
import { accountKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import type { MyWork } from "../types";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { ICON_SIZE } from "./ui/icon";
import { formatTaskDueDate } from "../web/format";
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
  if (query.isLoading) return null;
  return (
    <section aria-labelledby="my-work-heading" className="rm-my-work">
      <PanelHeading description={work.total_count ? `${work.total_count} waiting${work.overdue_task_count ? `, ${work.overdue_task_count} overdue` : ""}` : null} icon={Checklist} id="my-work-heading" title="My work" />
      {work.total_count === 0 ? <p className="shared-muted-copy">Nothing is waiting for you. Tasks assigned to you, files you own that need review and new mentions appear here.</p> : (
        <ul className="rm-my-work-list">
          {work.tasks.map((task) => <li key={`task-${task.id}`}><Button className="rm-my-work-item" onClick={() => navigate(`/workspaces/${encodeURIComponent(task.workspace_id)}/tasks`)} type="button" variant="secondary"><span><strong>{task.title}</strong><small>Task · {task.priority} priority{task.due_at ? ` · due ${formatTaskDueDate(task.due_at)}` : ""}</small></span><ChevronRight size={ICON_SIZE.md} /></Button></li>)}
          {work.reviews.map((review) => <li key={`review-${review.artifact_id}`}><Button className="rm-my-work-item" onClick={() => navigate(`/workspaces/${encodeURIComponent(review.workspace_id)}/review`)} type="button" variant="secondary"><span><strong>{review.title}</strong><small>Review · {review.workspace_name}{review.reason ? ` · ${review.reason}` : ""}</small></span><ChevronRight size={ICON_SIZE.md} /></Button></li>)}
          {work.mentions.map((mention) => <li key={`mention-${mention.id}`}><Button className="rm-my-work-item" onClick={() => navigate(mention.href)} type="button" variant="secondary"><span><strong>{mention.title}</strong><small>Mention{mention.body ? ` · ${mention.body}` : ""}</small></span><ChevronRight size={ICON_SIZE.md} /></Button></li>)}
        </ul>
      )}
    </section>
  );
}
