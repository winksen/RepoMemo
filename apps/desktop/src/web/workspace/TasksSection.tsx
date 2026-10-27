import { type FormEvent } from "react";
import { IconChecklist as Checklist, IconFileText as FileText, IconFilter as Filter, IconPlus as Plus, IconSearch as Search, IconTrash as Trash } from "@tabler/icons-react";
import { createSharedCollaborationTask, createSharedTaskChecklistItem, deleteSharedCollaborationTask, getSharedWorkspaceMetrics, listSharedCollaborationTasks, listSharedTaskChecklist, toggleSharedTaskChecklistItem, updateSharedCollaborationTask } from "../../lib/sharedApi";
import { type CollaborationTask, type CollaborationTaskPriority, type CollaborationTaskStatus, type TaskChecklistItem } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Checkbox } from "../../components/ui/checkbox";
import { EmptyState } from "../../components/ui/empty-state";
import { FeedbackNotice } from "../../components/ui/feedback-notice";
import { Pagination, paginate } from "../../components/ui/pagination";
import { Field } from "../../components/ui/field";
import { IconButton } from "../../components/ui/icon-button";
import { Combobox } from "../../components/ui/combobox";
import { useConfirm } from "../../components/ui/confirm-dialog";
import { Dropdown } from "../../components/ui/dropdown";
import { Input } from "../../components/ui/input";
import { SearchField } from "../../components/ui/search-field";
import { MentionTextarea } from "../../components/MentionTextarea";
import { TASK_COLUMNS } from "../constants";
import { apiMessage, formatTaskDueDate, isTaskOverdue } from "../format";
import { useAsyncAction } from "../../lib/useAsyncAction";
import { useArtifactPicker, useMembers, useMetrics, useTasks } from "./data";
import { type WorkspacePage, sectionState } from "./page";
import { ICON_SIZE } from "../../components/ui/icon";

export function TasksSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, session, canWrite, canManageMembers, onOpenArtifact } = page;
  const { isBusy: isSubmitting, run } = useAsyncAction();
  const useS = sectionState(page.store, "tasks");
  const confirm = useConfirm();
  const [taskChecklists, setTaskChecklists] = useS<Record<string, TaskChecklistItem[]>>("taskChecklists", {});
  const [expandedTaskId, setExpandedTaskId] = useS<string | null>("expandedTaskId", null);
  const [checklistBody, setChecklistBody] = useS("checklistBody", "");
  const [taskQuery, setTaskQuery] = useS("taskQuery", "");
  // Each column pages on its own once it holds more than 24 tasks.
  const [taskPages, setTaskPages] = useS<Record<string, number>>("taskPages", {});
  const [taskAssigneeFilter, setTaskAssigneeFilter] = useS("taskAssigneeFilter", "all");
  const [taskSort, setTaskSort] = useS("taskSort", "priority");
  const [taskTitle, setTaskTitle] = useS("taskTitle", "");
  const [taskDescription, setTaskDescription] = useS("taskDescription", "");
  const [taskPriority, setTaskPriority] = useS<CollaborationTaskPriority>("taskPriority", "medium");
  const [taskAssigneeId, setTaskAssigneeId] = useS("taskAssigneeId", "");
  const [taskArtifactId, setTaskArtifactId] = useS("taskArtifactId", "");
  const [taskDueDate, setTaskDueDate] = useS("taskDueDate", "");
  const picker = useArtifactPicker(accessToken, workspaceId);
  const [members] = useMembers(accessToken, workspaceId, canManageMembers);
  const [tasks, setTasks, tasksQuery] = useTasks(accessToken, workspaceId, true);
  const [, setWorkspaceMetrics] = useMetrics(accessToken, workspaceId);
  const visibleTasks = tasks.filter((task) => {
    const normalizedQuery = taskQuery.trim().toLowerCase();
    const matchesQuery = !normalizedQuery || `${task.title} ${task.description}`.toLowerCase().includes(normalizedQuery);
    const matchesAssignee = taskAssigneeFilter === "all"
      || (taskAssigneeFilter === "unassigned" ? !task.assignee : task.assignee?.id === taskAssigneeFilter);
    return matchesQuery && matchesAssignee;
  }).sort((left, right) => {
    if (taskSort === "title") return left.title.localeCompare(right.title) || left.id.localeCompare(right.id);
    if (taskSort === "due") return (left.due_at ?? "9999").localeCompare(right.due_at ?? "9999") || left.title.localeCompare(right.title);
    const priority = { urgent: 0, high: 1, medium: 2, low: 3 };
    return priority[left.priority] - priority[right.priority] || (left.due_at ?? "9999").localeCompare(right.due_at ?? "9999") || left.title.localeCompare(right.title);
  });
  async function addTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await run(async () => {
      await createSharedCollaborationTask(accessToken, workspace.workspace.id, {
        title: taskTitle,
        description: taskDescription,
        status: "open",
        priority: taskPriority,
        assigneeUserId: taskAssigneeId || undefined,
        artifactId: taskArtifactId || undefined,
        dueAt: taskDueDate ? `${taskDueDate}T23:59:59Z` : undefined,
      });
      setTaskTitle(""); setTaskDescription(""); setTaskPriority("medium"); setTaskAssigneeId(""); setTaskArtifactId(""); setTaskDueDate("");
      setTasks(await listSharedCollaborationTasks(accessToken, workspace.workspace.id));
      setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id));
    }, { success: "Task created." });
  }

  async function reviseTask(task: CollaborationTask, changes: { status?: CollaborationTaskStatus; priority?: CollaborationTaskPriority; assigneeUserId?: string | null }) {
    await run(async () => {
      const updated = await updateSharedCollaborationTask(accessToken, task.id, {
        title: task.title,
        description: task.description,
        status: changes.status ?? task.status,
        priority: changes.priority ?? task.priority,
        assigneeUserId: changes.assigneeUserId === null ? undefined : changes.assigneeUserId ?? task.assignee?.id,
        artifactId: task.artifact_id ?? undefined,
        dueAt: task.due_at ?? undefined,
      });
      setTasks((current) => current.map((entry) => entry.id === updated.id ? updated : entry));
      setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id));
    });
  }

  async function removeTask(task: CollaborationTask) {
    if (!(await confirm({ action: "Delete", danger: true, title: `Delete “${task.title}”?` }))) return;
    await run(async () => {
      await deleteSharedCollaborationTask(accessToken, task.id);
      setTasks((current) => current.filter((entry) => entry.id !== task.id));
      setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id));
    }, { success: "Task deleted." });
  }

  async function toggleTaskChecklist(task: CollaborationTask) {
    if (expandedTaskId === task.id) { setExpandedTaskId(null); return; }
    await run(async () => {
      const checklist = await listSharedTaskChecklist(accessToken, task.id);
      setTaskChecklists((current) => ({ ...current, [task.id]: checklist }));
      setExpandedTaskId(task.id);
    });
  }

  async function addChecklistItem(task: CollaborationTask, event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await run(async () => {
      const item = await createSharedTaskChecklistItem(accessToken, task.id, checklistBody);
      setTaskChecklists((current) => ({ ...current, [task.id]: [...(current[task.id] ?? []), item] }));
      setChecklistBody("");
    }, { success: "Checklist item added." });
  }

  async function toggleChecklistItem(item: TaskChecklistItem) {
    await run(async () => {
      const updated = await toggleSharedTaskChecklistItem(accessToken, item.id, !item.completed_at);
      setTaskChecklists((current) => ({ ...current, [item.task_id]: (current[item.task_id] ?? []).map((entry) => entry.id === updated.id ? updated : entry) }));
    });
  }

  return (
            <div className="shared-task-workspace">
              <PanelHeading description={<>{tasks.length} tasks</>} icon={Checklist} title="Team action board" />
              <div className="shared-task-toolbar"><SearchField aria-label="Search tasks" onValueChange={setTaskQuery} placeholder="Search action items" value={taskQuery} /><Dropdown aria-label="Filter tasks by assignee" onValueChange={setTaskAssigneeFilter} options={[{ label: "Everyone", value: "all" }, { label: "Unassigned", value: "unassigned" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={taskAssigneeFilter} /><Dropdown aria-label="Sort tasks" onValueChange={(value) => { setTaskSort(value); setTaskPages({}); }} options={[{ label: "Priority first", value: "priority" }, { label: "Due soonest", value: "due" }, { label: "Title A–Z", value: "title" }]} value={taskSort} /></div>
              {tasksQuery?.isLoading ? <FeedbackNotice kind="loading" title="Loading action items" /> : null}{tasksQuery?.error ? <FeedbackNotice action="Try again" detail={apiMessage(tasksQuery.error)} kind="error" onAction={() => void tasksQuery.refetch()} title="Action items could not be loaded" /> : null}{canWrite ? <form className="shared-task-create" onSubmit={addTask}><div className="shared-task-create-heading"><div><Plus size={ICON_SIZE.md} /><div><strong>Create an action item</strong><span>Assign work and tie it back to the evidence that prompted it.</span></div></div></div><Field label="Title" required><Input onChange={(event) => setTaskTitle(event.target.value)} placeholder="What needs to happen?" value={taskTitle} /></Field><Field label="Description"><MentionTextarea accessToken={accessToken} onValueChange={setTaskDescription} placeholder="Context, acceptance criteria, next steps, or type @ to mention a teammate" value={taskDescription} workspaceId={workspaceId} /></Field><div className="shared-task-create-fields"><Field label="Priority">{(control) => <Dropdown id={control.id} onValueChange={(value) => setTaskPriority(value as CollaborationTaskPriority)} options={[{ label: "Low priority", value: "low" }, { label: "Medium priority", value: "medium" }, { label: "High priority", value: "high" }, { label: "Urgent", value: "urgent" }]} value={taskPriority} />}</Field><Field label="Assignee">{(control) => <Dropdown id={control.id} onValueChange={(value) => setTaskAssigneeId(value === "__unassigned__" ? "" : value)} options={[{ label: "Unassigned", value: "__unassigned__" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={taskAssigneeId || "__unassigned__"} />}</Field><Field label="Related evidence">{(control) => <Combobox aria-label="Related evidence" emptyLabel="No file matches" id={control.id} noneLabel="No evidence link" onSearch={picker.search} onValueChange={setTaskArtifactId} options={picker.files.map((artifact) => ({ label: artifact.title, value: artifact.id }))} value={taskArtifactId} />}</Field><Field label="Due date"><Input onChange={(event) => setTaskDueDate(event.target.value)} type="date" value={taskDueDate} /></Field></div><Button busy={isSubmitting} type="submit" variant="main">Create task</Button></form> : null}
              {visibleTasks.length ? <div className="shared-task-board">{TASK_COLUMNS.map((column) => { const columnTasks = visibleTasks.filter((task) => task.status === column.status); const columnPage = taskPages[column.status] ?? 0; return <section className="shared-task-column" key={column.status}><div className="shared-task-column-heading"><h3>{column.label}</h3><span>{columnTasks.length}</span></div><div className="shared-task-column-list">{columnTasks.length ? paginate(columnTasks, columnPage).map((task) => <article className={`shared-task-card priority-${task.priority}${isTaskOverdue(task) ? " overdue" : ""}`} key={task.id}><div className="shared-task-card-heading"><span>{task.priority}</span>{task.due_at ? <time dateTime={task.due_at}>{formatTaskDueDate(task.due_at)}</time> : null}</div><h4>{task.title}</h4>{task.description ? <p>{task.description}</p> : null}<div className="shared-task-context"><span>{task.assignee?.display_name ?? "Unassigned"}</span>{task.artifact_id ? <Button onClick={() => onOpenArtifact(task.artifact_id!)} variant="secondary"><FileText size={ICON_SIZE.sm} /> Evidence</Button> : null}</div>{canWrite ? <div className="shared-task-controls"><Dropdown aria-label={`Status for ${task.title}`} onValueChange={(value) => void reviseTask(task, { status: value as CollaborationTaskStatus })} options={TASK_COLUMNS.map((option) => ({ label: option.label, value: option.status }))} value={task.status} /><Dropdown aria-label={`Assignee for ${task.title}`} onValueChange={(value) => void reviseTask(task, { assigneeUserId: value === "__unassigned__" ? null : value })} options={[{ label: "Unassigned", value: "__unassigned__" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={task.assignee?.id ?? "__unassigned__"} /><Dropdown aria-label={`Priority for ${task.title}`} onValueChange={(value) => void reviseTask(task, { priority: value as CollaborationTaskPriority })} options={[{ label: "Low", value: "low" }, { label: "Medium", value: "medium" }, { label: "High", value: "high" }, { label: "Urgent", value: "urgent" }]} value={task.priority} />{task.created_by.id === session.user.id || workspace.role === "owner" || workspace.role === "admin" ? <IconButton disabled={isSubmitting} label={`Delete ${task.title}`} onClick={() => void removeTask(task)}><Trash size={ICON_SIZE.sm} /></IconButton> : null}</div> : null}</article>) : <p>No tasks here.</p>}</div><Pagination onPageChange={(next) => setTaskPages((current) => ({ ...current, [column.status]: next }))} page={columnPage} total={columnTasks.length} /></section>; })}</div> : <EmptyState action={taskQuery || taskAssigneeFilter !== "all" ? <Button onClick={() => { setTaskQuery(""); setTaskAssigneeFilter("all"); }} type="button" variant="secondary">Clear filters</Button> : null} description={taskQuery || taskAssigneeFilter !== "all" ? "Adjust your search or assignee to see more action items." : "Create an action item to assign the next step."} icon={<Checklist size={25} />} title={taskQuery || taskAssigneeFilter !== "all" ? "No action items match" : "No action items yet"} />}
              {expandedTaskId ? (() => { const task = tasks.find((entry) => entry.id === expandedTaskId); return task ? <section className="shared-task-checklist-panel"><PanelHeading actions={<Button onClick={() => setExpandedTaskId(null)} variant="secondary">Close</Button>} icon={Checklist} level={3} title={<>{task.title} checklist</>} />{(taskChecklists[task.id] ?? []).length ? <div className="shared-task-checklist-list">{(taskChecklists[task.id] ?? []).map((item) => <Checkbox checked={Boolean(item.completed_at)} disabled={!canWrite || isSubmitting} key={item.id} label={item.body} onCheckedChange={() => void toggleChecklistItem(item)} />)}</div> : <EmptyState description="Break the work into clear, checkable outcomes." title="No steps yet" />}{canWrite ? <form className="shared-task-checklist-form" onSubmit={(event) => void addChecklistItem(task, event)}><Field label="New checklist step" required><Input onChange={(event) => setChecklistBody(event.target.value)} placeholder="Add a completion step" value={checklistBody} /></Field><Button busy={isSubmitting} type="submit" variant="secondary">Add step</Button></form> : null}</section> : null; })() : null}
              <div className="shared-task-checklist-select"><Combobox aria-label="Open a task checklist" emptyLabel="No task matches" onValueChange={(value) => { const task = tasks.find((entry) => entry.id === value); if (task) void toggleTaskChecklist(task); }} options={tasks.map((task) => ({ label: task.title, value: task.id }))} placeholder="Open task checklist" value={expandedTaskId ?? ""} /></div>
            </div>
  );
}
