-- Project updates (V0.7.17): a person's dated statement of a project's health, with why they chose it and what it points at.
-- The health (`on_track`, `at_risk`, `blocked`) is always chosen by the person who publishes: nothing here computes it. An update
-- keeps the counts as they were when it was published and what changed since the one before, so it can be read later as it
-- was. It is never edited; a person can retract it, which hides it from the default list and keeps the record.
CREATE TABLE IF NOT EXISTS project_updates (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES workspace_projects(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  author_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  health TEXT NOT NULL CHECK (health IN ('on_track', 'at_risk', 'blocked')),
  summary TEXT NOT NULL,
  reason TEXT NOT NULL,
  snapshot_json TEXT NOT NULL,
  changes_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  retracted_at TEXT,
  retracted_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_project_updates_project ON project_updates (project_id, created_at DESC);

-- What an update points at: milestones and tasks of the project (the blockers), and evidence or memory cards. The label is the
-- name at the time of publishing; the name now and whether the target still exists are read when the update is read.
CREATE TABLE IF NOT EXISTS project_update_links (
  update_id TEXT NOT NULL REFERENCES project_updates(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('milestone', 'task', 'evidence', 'memory_card')),
  target_id TEXT NOT NULL,
  label TEXT NOT NULL,
  PRIMARY KEY (update_id, kind, target_id)
);
