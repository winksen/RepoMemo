import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const data = vi.hoisted(() => ({ tasks: [] as unknown[] }));
vi.mock("../lib/sharedApi", () => ({
  createSharedCollaborationTask: vi.fn(),
  createSharedTaskChecklistItem: vi.fn(),
  deleteSharedCollaborationTask: vi.fn(),
  getSharedWorkspaceMetrics: vi.fn(),
  listSharedCollaborationTasks: vi.fn(),
  listSharedTaskChecklist: vi.fn(),
  toggleSharedTaskChecklistItem: vi.fn(),
  updateSharedCollaborationTask: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));
vi.mock("../web/workspace/data", () => ({
  useArtifactPicker: () => ({ files: [{ id: "a1", title: "Runbook" }], search: () => undefined, find: () => undefined, known: () => [] }),
  useMembers: () => [[{ user: { id: "u1", display_name: "Ada" } }]],
  useMetrics: () => [null, vi.fn()],
  useTasks: () => [data.tasks, vi.fn()],
}));

import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { TasksSection } from "../web/workspace/TasksSection";
import { makePage } from "./workspacePage";

afterEach(() => { cleanup(); data.tasks = []; });

const task = { id: "t1", title: "Fix login", description: "", status: "open", priority: "medium", assignee: null, artifact_id: null, due_at: null, created_by: { id: "u1" } };

function renderTasks(store = new Map<string, unknown>()) {
  return render(<ConfirmProvider><TasksSection page={makePage({ section: "tasks", store })} /></ConfirmProvider>);
}

describe("tasks on the shared pieces", () => {
  it("labels the fields of the create form and marks the title as required", () => {
    renderTasks();
    expect((screen.getByLabelText("Title") as HTMLInputElement).required).toBe(true);
    for (const label of ["Description", "Priority", "Assignee", "Related evidence", "Due date"]) {
      expect(screen.getByLabelText(label).closest(".rm-field")).not.toBeNull();
    }
    expect((screen.getByLabelText("Due date") as HTMLInputElement).type).toBe("date");
  });

  it("shows checklist steps as shared checkboxes with their labels and an empty state without steps", () => {
    data.tasks = [task];
    const withSteps = new Map<string, unknown>([["tasks.expandedTaskId", "t1"], ["tasks.taskChecklists", { t1: [{ id: "c1", task_id: "t1", body: "Write tests", completed_at: null }, { id: "c2", task_id: "t1", body: "Ship it", completed_at: "2026-10-01T00:00:00Z" }] }]]);
    renderTasks(withSteps);
    expect(screen.getByRole("checkbox", { name: "Write tests" }).getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("checkbox", { name: "Ship it" }).getAttribute("aria-checked")).toBe("true");
    expect((screen.getByLabelText("New checklist step") as HTMLInputElement).required).toBe(true);
    cleanup();
    renderTasks(new Map<string, unknown>([["tasks.expandedTaskId", "t1"]]));
    expect(screen.getByText("No steps yet").closest(".rm-empty-state")).not.toBeNull();
  });

  it("deletes a task from an icon button that names it", () => {
    data.tasks = [task];
    renderTasks();
    expect(screen.getByRole("button", { name: "Delete Fix login" }).className).toContain("rm-icon-button");
  });

  it("sorts visible tasks by priority and lets the user choose title order", () => {
    data.tasks = [{ ...task, id: "t2", title: "Alpha", priority: "low" }, { ...task, id: "t3", title: "Zulu", priority: "urgent" }];
    renderTasks();
    expect(screen.getByRole("combobox", { name: "Sort tasks" })).toBeTruthy();
    expect(Array.from(document.querySelectorAll(".shared-task-card h4"), (title) => title.textContent)).toEqual(["Zulu", "Alpha"]);
    cleanup();
    renderTasks(new Map<string, unknown>([["tasks.taskSort", "title"]]));
    expect(Array.from(document.querySelectorAll(".shared-task-card h4"), (title) => title.textContent)).toEqual(["Alpha", "Zulu"]);
  });
});
