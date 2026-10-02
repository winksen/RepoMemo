-- Nested folders for organizing shared evidence. Depth is limited in code
-- (see `MAX_FOLDER_DEPTH` in the domain crate), not in the schema.
CREATE TABLE IF NOT EXISTS folders (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  parent_id TEXT,
  name TEXT NOT NULL,
  created_by TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_folders_workspace ON folders (workspace_id, parent_id);

ALTER TABLE artifacts ADD COLUMN folder_id TEXT;
