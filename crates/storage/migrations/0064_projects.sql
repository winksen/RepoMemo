-- Projects (V0.7.1): a named piece of work inside one workspace, with an owner, a goal, a lifecycle and a target date.
-- A project grants no access: who can read or change it is decided by the workspace's membership alone. A task may belong
-- to one project; a task with no project (every task made before 0.7) keeps working unchanged.
CREATE TABLE IF NOT EXISTS workspace_projects (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  goal TEXT NOT NULL DEFAULT '',
  -- `active`, `on_hold`, `done` or `archived`; archiving keeps the project and its tasks readable
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'on_hold', 'done', 'archived')),
  owner_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  -- a calendar date (YYYY-MM-DD): a statement of intent, not a schedule the tasks follow
  target_date TEXT,
  archived_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workspace_projects_workspace ON workspace_projects (workspace_id, status, updated_at DESC);

-- A task belongs to at most one project of its own workspace (checked when saved); a removed project leaves its tasks.
ALTER TABLE workspace_tasks ADD COLUMN project_id TEXT REFERENCES workspace_projects(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_workspace_tasks_project ON workspace_tasks (project_id, status);
