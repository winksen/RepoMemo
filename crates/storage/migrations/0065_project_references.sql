-- A project's brief (V0.7.2): the decision memories and evidence a project cites. Nothing is copied: a reference holds only
-- the target's id and a short note, the target's title is read when the brief is read, and a target that is gone shows as
-- unavailable. A reference never widens access: the brief is read with the workspace's own rights.
CREATE TABLE IF NOT EXISTS project_references (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES workspace_projects(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  -- `memory_card` or `evidence` (a file or pasted note); the target is looked up in this workspace only
  kind TEXT NOT NULL CHECK (kind IN ('memory_card', 'evidence')),
  target_id TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  added_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  UNIQUE (project_id, kind, target_id)
);

CREATE INDEX IF NOT EXISTS idx_project_references_target ON project_references (workspace_id, kind, target_id);
