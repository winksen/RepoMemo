-- Opaque, rotating refresh tokens. Only the SHA-256 of the token is stored.
-- `revoked_at` is set when a token is rotated or its owner signs out/changes password;
-- presenting a revoked token again is treated as theft and revokes every token of the user.
CREATE TABLE IF NOT EXISTS refresh_tokens (
  token_hash TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user ON refresh_tokens (user_id);
