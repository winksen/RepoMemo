import { type FormEvent } from "react";
import { IconChecklist as Checklist, IconFileText as FileText, IconFilter as Filter, IconPlus as Plus, IconSearch as Search, IconTrash as Trash } from "@tabler/icons-react";
import { createSharedCollaborationTask, createSharedTaskChecklistItem, deleteSharedCollaborationTask, getSharedWorkspaceMetrics, listSharedCollaborationTasks, listSharedTaskChecklist, toggleSharedTaskChecklistItem, updateSharedCollaborationTask } from "../../lib/sharedApi";
import { type CollaborationTask, type CollaborationTaskPriority, type CollaborationTaskStatus, type TaskChecklistItem } from "../../types";
import { Button } from "../../components/ui/button";
import { Dropdown } from "../../components/ui/dropdown";
import { Input } from "../../components/ui/input";
import { Textarea } from "../../components/ui/textarea";
import { TASK_COLUMNS } from "../constants";
import { apiMessage, formatTaskDueDate, isTaskOverdue } from "../format";
import { useArtifacts, useMembers, useMetrics, useTasks } from "./data";
import { type WorkspacePage, sectionState } from "./page";

export function TasksSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, session, canWrite, canManageMembers, isSubmitting, setIsSubmitting, setError, onOpenArtifact } = page;
  const useS = sectionState(page.store, "tasks");
  const [taskChecklists, setTaskChecklists] = useS<Record<string, TaskChecklistItem[]>>("taskChecklists", {});
  const [expandedTaskId, setExpandedTaskId] = useS<string | null>("expandedTaskId", null);
  const [checklistBody, setChecklistBody] = useS("checklistBody", "");
  const [taskQuery, setTaskQuery] = useS("taskQuery", "");
  const [taskAssigneeFilter, setTaskAssigneeFilter] = useS("taskAssigneeFilter", "all");
  const [taskTitle, setTaskTitle] = useS("taskTitle", "");
  const [taskDescription, setTaskDescription] = useS("taskDescription", "");
  const [taskPriority, setTaskPriority] = useS<CollaborationTaskPriority>("taskPriority", "medium");
  const [taskAssigneeId, setTaskAssigneeId] = useS("taskAssigneeId", "");
  const [taskArtifactId, setTaskArtifactId] = useS("taskArtifactId", "");
  const [taskDueDate, setTaskDueDate] = useS("taskDueDate", "");
  const [artifacts] = useArtifacts(accessToken, workspaceId);
  const [members] = useMembers(accessToken, workspaceId, canManageMembers);
  const [tasks, setTasks] = useTasks(accessToken, workspaceId, true);
  const [, setWorkspaceMetrics] = useMetrics(accessToken, workspaceId);
  const visibleTasks = tasks.filter((task) => {
    const normalizedQuery = taskQuery.trim().toLowerCase();
    const matchesQuery = !normalizedQuery || `${task.title} ${task.description}`.toLowerCase().includes(normalizedQuery);
    const matchesAssignee = taskAssigneeFilter === "all"
      || (taskAssigneeFilter === "unassigned" ? !task.assignee : task.assignee?.id === taskAssigneeFilter);
    return matchesQuery && matchesAssignee;
  });
  async function addTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
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
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function reviseTask(task: CollaborationTask, changes: { status?: CollaborationTaskStatus; priority?: CollaborationTaskPriority; assigneeUserId?: string | null }) {
    setIsSubmitting(true); setError(null);
    try {
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
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeTask(task: CollaborationTask) {
    if (!window.confirm(`Delete “${task.title}”?`)) return;
    setIsSubmitting(true); setError(null);
    try {
      await deleteSharedCollaborationTask(accessToken, task.id);
      setTasks((current) => current.filter((entry) => entry.id !== task.id));
      setWorkspaceMetrics(await getSharedWorkspaceMetrics(accessToken, workspace.workspace.id));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function toggleTaskChecklist(task: CollaborationTask) {
    if (expandedTaskId === task.id) { setExpandedTaskId(null); return; }
    setIsSubmitting(true); setError(null);
    try { const checklist = await listSharedTaskChecklist(accessToken, task.id); setTaskChecklists((current) => ({ ...current, [task.id]: checklist })); setExpandedTaskId(task.id); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function addChecklistItem(task: CollaborationTask, event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setIsSubmitting(true); setError(null);
    try { const item = await createSharedTaskChecklistItem(accessToken, task.id, checklistBody); setTaskChecklists((current) => ({ ...current, [task.id]: [...(current[task.id] ?? []), item] })); setChecklistBody(""); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function toggleChecklistItem(item: TaskChecklistItem) {
    setIsSubmitting(true); setError(null);
    try { const updated = await toggleSharedTaskChecklistItem(accessToken, item.id, !item.completed_at); setTaskChecklists((current) => ({ ...current, [item.task_id]: (current[item.task_id] ?? []).map((entry) => entry.id === updated.id ? updated : entry) })); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
            <div className="shared-task-workspace">
              <div className="shared-panel-heading"><div><Checklist size={18} /><h2>Team action board</h2></div><span>{tasks.length} tasks</span></div>
              <div className="shared-task-toolbar"><Input aria-label="Search tasks" onChange={(event) => setTaskQuery(event.target.value)} placeholder="Search action items" value={taskQuery} /><Dropdown aria-label="Filter tasks by assignee" onValueChange={setTaskAssigneeFilter} options={[{ label: "Everyone", value: "all" }, { label: "Unassigned", value: "unassigned" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={taskAssigneeFilter} /></div>
              {canWrite ? <form className="shared-task-create" onSubmit={addTask}><div className="shared-task-create-heading"><div><Plus size={17} /><div><strong>Create an action item</strong><span>Assign work and tie it back to the evidence that prompted it.</span></div></div></div><Input aria-label="Task title" onChange={(event) => setTaskTitle(event.target.value)} placeholder="What needs to happen?" required value={taskTitle} /><Textarea aria-label="Task description" onChange={(event) => setTaskDescription(event.target.value)} placeholder="Context, acceptance criteria, or next steps" value={taskDescription} /><div className="shared-task-create-fields"><Dropdown aria-label="Task priority" onValueChange={(value) => setTaskPriority(value as CollaborationTaskPriority)} options={[{ label: "Low priority", value: "low" }, { label: "Medium priority", value: "medium" }, { label: "High priority", value: "high" }, { label: "Urgent", value: "urgent" }]} value={taskPriority} /><Dropdown aria-label="Task assignee" onValueChange={(value) => setTaskAssigneeId(value === "__unassigned__" ? "" : value)} options={[{ label: "Unassigned", value: "__unassigned__" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={taskAssigneeId || "__unassigned__"} /><Dropdown aria-label="Related evidence" onValueChange={(value) => setTaskArtifactId(value === "__none__" ? "" : value)} options={[{ label: "No evidence link", value: "__none__" }, ...artifacts.map((artifact) => ({ label: artifact.title, value: artifact.id }))]} value={taskArtifactId || "__none__"} /><Input aria-label="Task due date" onChange={(event) => setTaskDueDate(event.target.value)} type="date" value={taskDueDate} /></div><Button disabled={isSubmitting} type="submit" variant="main">Create task</Button></form> : null}
              {visibleTasks.length ? <div className="shared-task-board">{TASK_COLUMNS.map((column) => { const columnTasks = visibleTasks.filter((task) => task.status === column.status); return <section className="shared-task-column" key={column.status}><div className="shared-task-column-heading"><h3>{column.label}</h3><span>{columnTasks.length}</span></div><div className="shared-task-column-list">{columnTasks.length ? columnTasks.map((task) => <article className={`shared-task-card priority-${task.priority}${isTaskOverdue(task) ? " overdue" : ""}`} key={task.id}><div className="shared-task-card-heading"><span>{task.priority}</span>{task.due_at ? <time dateTime={task.due_at}>{formatTaskDueDate(task.due_at)}</time> : null}</div><h4>{task.title}</h4>{task.description ? <p>{task.description}</p> : null}<div className="shared-task-context"><span>{task.assignee?.display_name ?? "Unassigned"}</span>{task.artifact_id ? <Button onClick={() => onOpenArtifact(task.artifact_id!)} type="button" variant="secondary"><FileText size={14} /> Evidence</Button> : null}</div>{canWrite ? <div className="shared-task-controls"><Dropdown aria-label={`Status for ${task.title}`} onValueChange={(value) => void reviseTask(task, { status: value as CollaborationTaskStatus })} options={TASK_COLUMNS.map((option) => ({ label: option.label, value: option.status }))} value={task.status} /><Dropdown aria-label={`Assignee for ${task.title}`} onValueChange={(value) => void reviseTask(task, { assigneeUserId: value === "__unassigned__" ? null : value })} options={[{ label: "Unassigned", value: "__unassigned__" }, ...members.map((member) => ({ label: member.user.display_name, value: member.user.id }))]} value={task.assignee?.id ?? "__unassigned__"} /><Dropdown aria-label={`Priority for ${task.title}`} onValueChange={(value) => void reviseTask(task, { priority: value as CollaborationTaskPriority })} options={[{ label: "Low", value: "low" }, { label: "Medium", value: "medium" }, { label: "High", value: "high" }, { label: "Urgent", value: "urgent" }]} value={task.priority} />{task.created_by.id === session.user.id || workspace.role === "owner" || workspace.role === "admin" ? <Button aria-label={`Delete ${task.title}`} disabled={isSubmitting} onClick={() => void removeTask(task)} type="button" variant="secondary"><Trash size={15} /></Button> : null}</div> : null}</article>) : <p>No tasks here.</p>}</div></section>; })}</div> : <div className="shared-empty-state"><Checklist size={25} /><strong>No action items match</strong><span>Create a task or adjust the current filters.</span></div>}
              {expandedTaskId ? (() => { const task = tasks.find((entry) => entry.id === expandedTaskId); return task ? <section className="shared-task-checklist-panel"><div className="shared-panel-heading"><div><Checklist size={17} /><h3>{task.title} checklist</h3></div><Button onClick={() => setExpandedTaskId(null)} type="button" variant="secondary">Close</Button></div>{(taskChecklists[task.id] ?? []).length ? <div className="shared-task-checklist-list">{(taskChecklists[task.id] ?? []).map((item) => <label key={item.id}><input checked={Boolean(item.completed_at)} disabled={!canWrite || isSubmitting} onChange={() => void toggleChecklistItem(item)} type="checkbox" /><span>{item.body}</span></label>)}</div> : <p className="shared-muted-copy">No steps yet. Break the work into clear, checkable outcomes.</p>}{canWrite ? <form className="shared-task-checklist-form" onSubmit={(event) => void addChecklistItem(task, event)}><Input aria-label="New checklist step" onChange={(event) => setChecklistBody(event.target.value)} placeholder="Add a completion step" required value={checklistBody} /><Button disabled={isSubmitting} type="submit" variant="secondary">Add step</Button></form> : null}</section> : null; })() : null}
              <div className="shared-task-checklist-select"><Dropdown aria-label="Open a task checklist" onValueChange={(value) => { const task = tasks.find((entry) => entry.id === value); if (task) void toggleTaskChecklist(task); }} options={[{ label: "Open task checklist", value: "__none__" }, ...tasks.map((task) => ({ label: task.title, value: task.id }))]} value={expandedTaskId ?? "__none__"} /></div>
            </div>
  );
}
