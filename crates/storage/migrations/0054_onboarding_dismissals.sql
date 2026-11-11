-- A person's dismissal of the workspace checklist (V0.6.1). The checklist itself is not stored: each step is
-- derived from what exists in the workspace, so only the choice to hide it is a row. Deleted with the person
-- or the workspace.
CREATE TABLE IF NOT EXISTS onboarding_dismissals (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  dismissed_at TEXT NOT NULL,
  PRIMARY KEY (user_id, workspace_id)
);
