-- Bounded trash for locally authored memory cards and uploaded files (V0.6.15). An item is kept for 30 days, up to 200 per
-- workspace, with what is needed to put it back under the same id. `content_hash` names a deleted file's stored content,
-- which blob garbage collection leaves alone while the item is here. Nothing from a repository or a connector is kept.
CREATE TABLE IF NOT EXISTS trash_items (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('memory_card', 'artifact')),
  item_id TEXT NOT NULL,
  title TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  content_hash TEXT,
  deleted_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  deleted_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_trash_items_workspace ON trash_items (workspace_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_trash_items_item ON trash_items (kind, item_id);
CREATE INDEX IF NOT EXISTS idx_trash_items_content ON trash_items (content_hash);
