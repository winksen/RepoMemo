-- The secret that signs push webhooks for one repository link (roadmap F16a,
-- V0.2.23). Sealed like provider keys: verifying a signature needs the secret
-- itself, so it cannot be hashed. `last_received_at` shows a sender works.
CREATE TABLE IF NOT EXISTS repo_webhooks (
  source_id TEXT PRIMARY KEY REFERENCES sources(id) ON DELETE CASCADE,
  sealed_secret TEXT NOT NULL,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  last_received_at TEXT
);
