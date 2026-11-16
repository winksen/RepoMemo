-- Workflow states (V0.7.11): a workspace can give its tasks named states (Review, QA, Waiting...), each mapped to one of the four
-- stable categories `open`, `in_progress`, `blocked` and `done`. The four built-in states always exist and are not rows here, so
-- every existing task keeps its status unchanged: a task with no `state_id` is in the built-in state of its status. A task's
-- `status` stays the category, so reports, the board and integrations read the explicit mapping and never a custom name.
CREATE TABLE IF NOT EXISTS workflow_states (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  -- the name in lower case, so two states cannot differ only by case (and none can repeat a built-in name)
  name_key TEXT NOT NULL,
  category TEXT NOT NULL CHECK (category IN ('open', 'in_progress', 'blocked', 'done')),
  position INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (workspace_id, name_key)
);

-- A state that still holds tasks cannot be removed: its tasks are moved first (the route asks where to).
ALTER TABLE workspace_tasks ADD COLUMN state_id TEXT REFERENCES workflow_states(id) ON DELETE RESTRICT;
CREATE INDEX IF NOT EXISTS idx_workspace_tasks_state ON workspace_tasks (state_id);
