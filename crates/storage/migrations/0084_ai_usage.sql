-- AI administration (V0.8.13): stored counts of the AI calls the server makes on its own for a workspace, and an optional limit of a
-- workspace's own. The counts survive a restart, so a spending limit stops calls the same way before and after one. `refused` counts
-- the calls a limit stopped. A workspace with no row in `workspace_ai_limits` uses the server's default; a limit of 0 is no limit.
CREATE TABLE IF NOT EXISTS ai_usage_daily (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  day TEXT NOT NULL,
  kind TEXT NOT NULL,
  calls INTEGER NOT NULL DEFAULT 0,
  refused INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (workspace_id, day, kind)
);

CREATE TABLE IF NOT EXISTS workspace_ai_limits (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
  daily_calls INTEGER NOT NULL CHECK (daily_calls >= 0),
  updated_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  updated_at TEXT NOT NULL
);
