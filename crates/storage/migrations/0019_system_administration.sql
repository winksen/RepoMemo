-- System administrators see every organization and workspace (acting as an
-- administrator in each) and manage the server itself.
ALTER TABLE users ADD COLUMN is_system_admin INTEGER NOT NULL DEFAULT 0;

-- Settings changed by a system administrator at run time. They override the
-- server's environment defaults; deleting a row restores the default.
CREATE TABLE IF NOT EXISTS system_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  updated_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

-- What system administrators did at the system level: role grants, settings,
-- ended sessions, maintenance runs. Kept independently of any workspace.
CREATE TABLE IF NOT EXISTS system_audit_events (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  action TEXT NOT NULL,
  detail TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_system_audit_events_created_at ON system_audit_events (created_at);
CREATE INDEX IF NOT EXISTS idx_workspace_activity_created_at ON workspace_activity (created_at);
