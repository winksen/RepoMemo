-- Saved task views (V0.7.12): a named set of task filters. A view stores only the filters, never task ids or what a reader may
-- see: it is evaluated each time against the reader's current rights, so a person who loses access to the workspace gets
-- nothing from a view they or a teammate saved. A view is private to its owner until the owner shares it with the workspace.
CREATE TABLE IF NOT EXISTS saved_task_views (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  query_json TEXT NOT NULL,
  shared INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_saved_task_views_workspace ON saved_task_views (workspace_id, owner_user_id);
