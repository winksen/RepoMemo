-- Task discussion (V0.7.4): comments that stay with their task, and explicit watching. A task's creator and assignee follow it
-- unless they mute it; anyone else follows it only by asking. Muting stops the optional comment notices, never the notice
-- that a task was assigned. Both are removed with the task.
CREATE TABLE IF NOT EXISTS task_comments (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES workspace_tasks(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  author_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  body TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_task_comments_task ON task_comments (task_id, created_at);

CREATE TABLE IF NOT EXISTS task_watchers (
  task_id TEXT NOT NULL REFERENCES workspace_tasks(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  -- `watching` follows the task; `muted` stops the optional notices even for its creator and assignee
  mode TEXT NOT NULL CHECK (mode IN ('watching', 'muted')),
  created_at TEXT NOT NULL,
  PRIMARY KEY (task_id, user_id)
);
