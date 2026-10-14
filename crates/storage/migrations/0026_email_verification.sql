-- Email verification (roadmap F13, V0.2.13). An account is verified unless it
-- is marked otherwise, so every account that exists keeps working: only an
-- account that registered on an open server without an invitation, or that
-- changed its address, waits for a link sent to that address.
ALTER TABLE users ADD COLUMN email_unverified_at TEXT;

CREATE TABLE IF NOT EXISTS email_verifications (
  token_hash TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  email TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_email_verifications_user ON email_verifications (user_id);
