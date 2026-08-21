CREATE TABLE IF NOT EXISTS workspace_tasks (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open',
  priority TEXT NOT NULL DEFAULT 'medium',
  assignee_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_by_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  artifact_id TEXT REFERENCES artifacts(id) ON DELETE SET NULL,
  due_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workspace_tasks_workspace_status
  ON workspace_tasks(workspace_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_workspace_tasks_assignee
  ON workspace_tasks(assignee_user_id, status, due_at);

CREATE TABLE IF NOT EXISTS artifact_comments (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  author_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifact_comments_artifact_created
  ON artifact_comments(artifact_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_artifact_comments_workspace_created
  ON artifact_comments(workspace_id, created_at DESC);
