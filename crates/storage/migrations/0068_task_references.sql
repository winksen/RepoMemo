-- Task context (V0.7.5): a task can cite several pieces of evidence and memory cards, not just one file. A reference holds the
-- target's id and a note; the title is read from the target when the task is read, so a deleted target shows as unavailable.
-- The old single link (`workspace_tasks.artifact_id`) stays as it is and is copied in as a reference, so nothing is lost and
-- every reader of the old field keeps working.
CREATE TABLE IF NOT EXISTS task_references (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES workspace_tasks(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  -- `evidence` (a file or pasted note) or `memory_card`; the target is looked up in this workspace only
  kind TEXT NOT NULL CHECK (kind IN ('evidence', 'memory_card')),
  target_id TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  added_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  UNIQUE (task_id, kind, target_id)
);

CREATE INDEX IF NOT EXISTS idx_task_references_target ON task_references (workspace_id, kind, target_id);

INSERT INTO task_references (id, task_id, workspace_id, kind, target_id, note, added_by_user_id, created_at)
  SELECT lower(hex(randomblob(16))), id, workspace_id, 'evidence', artifact_id, '', created_by_user_id, created_at
  FROM workspace_tasks WHERE artifact_id IS NOT NULL;
