-- Task hierarchy (V0.7.8): a task can have a parent task of the same workspace. A sub-task is a task of its own, with its own
-- assignee, status and discussion; a checklist step is not (it has none of these). Removing a parent leaves its sub-tasks as
-- ordinary tasks. Cycles and depth are checked when a task is saved, not here.
ALTER TABLE workspace_tasks ADD COLUMN parent_task_id TEXT REFERENCES workspace_tasks(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_workspace_tasks_parent ON workspace_tasks (parent_task_id);
