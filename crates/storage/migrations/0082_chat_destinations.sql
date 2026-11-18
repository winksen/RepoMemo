-- Chat destinations (V0.8.9): a place an administrator chose to send digests of a workspace to. The address (a Slack incoming
-- webhook) is itself a secret, so it is sealed and never returned. A digest is a count and a link to the signed-in app, never a
-- title or an excerpt. `chat_digest_log` is the ledger that keeps one digest of a kind to one destination for each window, so a
-- retry or a restart sends nothing twice.
CREATE TABLE IF NOT EXISTS chat_destinations (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (provider IN ('slack')),
  name TEXT NOT NULL,
  host TEXT NOT NULL,
  sealed_url TEXT NOT NULL,
  digests_json TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_sent_at TEXT,
  last_error TEXT
);

CREATE INDEX IF NOT EXISTS idx_chat_destinations_workspace ON chat_destinations (workspace_id);

CREATE TABLE IF NOT EXISTS chat_digest_log (
  destination_id TEXT NOT NULL REFERENCES chat_destinations(id) ON DELETE CASCADE,
  digest_kind TEXT NOT NULL,
  window_key TEXT NOT NULL,
  claimed_at TEXT NOT NULL,
  PRIMARY KEY (destination_id, digest_kind, window_key)
);
