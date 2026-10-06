-- What administrators did with each Workspace Health finding. A finding
-- with any row here stays hidden until its facts change (and with them its
-- fingerprint). The rows also measure, per detector, how often findings are
-- acted on rather than dismissed.
CREATE TABLE IF NOT EXISTS workspace_health_actions (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  fingerprint TEXT NOT NULL,
  detector TEXT NOT NULL,
  action TEXT NOT NULL,
  actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workspace_health_actions_workspace
  ON workspace_health_actions(workspace_id, fingerprint);
