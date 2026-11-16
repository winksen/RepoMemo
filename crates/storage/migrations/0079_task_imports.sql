-- CSV migration (V0.7.18): a ledger of the rows already imported into a workspace, so sending the same file again (a retry, a
-- second attempt after fixing other rows) creates nothing twice. A row's key is its id in the source (when the file has one) or
-- a fingerprint of its content within its import; the task survives the ledger and the ledger survives the task.
CREATE TABLE IF NOT EXISTS task_imports (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  source_key TEXT NOT NULL,
  task_id TEXT REFERENCES workspace_tasks(id) ON DELETE SET NULL,
  imported_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  imported_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, source_key)
);
