-- Dependencies (V0.7.9): "this task blocks that one". A dependency only informs: closing a blocker changes what the blocked task
-- shows, never its status. Self-links are refused here; cycles and links across workspaces are refused when one is added.
CREATE TABLE IF NOT EXISTS task_dependencies (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  blocker_task_id TEXT NOT NULL REFERENCES workspace_tasks(id) ON DELETE CASCADE,
  blocked_task_id TEXT NOT NULL REFERENCES workspace_tasks(id) ON DELETE CASCADE,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  UNIQUE (blocker_task_id, blocked_task_id),
  CHECK (blocker_task_id != blocked_task_id)
);

CREATE INDEX IF NOT EXISTS idx_task_dependencies_blocked ON task_dependencies (blocked_task_id);
CREATE INDEX IF NOT EXISTS idx_task_dependencies_blocker ON task_dependencies (blocker_task_id);
