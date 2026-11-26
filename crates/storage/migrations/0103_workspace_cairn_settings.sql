-- A workspace's CAIRN settings (V0.9.85): which of its organization's models it offers and which is its default, kept as one versioned
-- policy. A row exists only once a workspace administrator saved it; a workspace with none offers what its organization offers and
-- starts with the model it had. The settings travel with the workspace's archive, by the names models are known by, and a model the
-- importing server does not know is dropped with a warning.
CREATE TABLE workspace_cairn_settings (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
  policy_json TEXT NOT NULL DEFAULT '{}',
  version INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL,
  updated_by TEXT REFERENCES users(id) ON DELETE SET NULL
);
