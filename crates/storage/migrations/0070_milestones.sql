-- Milestones (V0.7.7): a named, dated group of one project's tasks. A date is a statement, not a schedule: changing it never
-- moves a task. A milestone is complete either by a rule (`all_tasks`: it has tasks and every one is done) or because a
-- person says so (`manual`). Removing a milestone leaves its tasks in the project.
CREATE TABLE IF NOT EXISTS project_milestones (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES workspace_projects(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  -- a calendar date (YYYY-MM-DD) in UTC, or none
  due_date TEXT,
  completion_rule TEXT NOT NULL DEFAULT 'all_tasks' CHECK (completion_rule IN ('all_tasks', 'manual')),
  -- set by a person for the `manual` rule; unused by `all_tasks`
  completed_at TEXT,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_project_milestones_project ON project_milestones (project_id, due_date);

ALTER TABLE workspace_tasks ADD COLUMN milestone_id TEXT REFERENCES project_milestones(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_workspace_tasks_milestone ON workspace_tasks (milestone_id, status);
