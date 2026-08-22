CREATE TABLE IF NOT EXISTS workspace_saved_searches (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  created_by_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  name TEXT NOT NULL,
  query TEXT NOT NULL,
  artifact_types_json TEXT NOT NULL DEFAULT '[]',
  languages_json TEXT NOT NULL DEFAULT '[]',
  source_ids_json TEXT NOT NULL DEFAULT '[]',
  result_limit INTEGER NOT NULL DEFAULT 20,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_workspace_saved_searches_workspace
  ON workspace_saved_searches(workspace_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS workspace_task_checklist_items (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES workspace_tasks(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  created_by_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  body TEXT NOT NULL,
  position INTEGER NOT NULL DEFAULT 0,
  completed_at TEXT,
  completed_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_workspace_task_checklist_items_task
  ON workspace_task_checklist_items(task_id, position, created_at);
