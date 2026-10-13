-- Personal access tokens gain a third scope, `system`: read and write plus
-- changes in the System area (review N2 residual). SQLite cannot alter a
-- CHECK constraint, so the table is rebuilt; nothing references it.
CREATE TABLE personal_access_tokens_v2 (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  prefix TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('read', 'write', 'system')),
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  last_used_at TEXT,
  revoked_at TEXT
);

INSERT INTO personal_access_tokens_v2 (id, user_id, name, token_hash, prefix, scope, created_at, expires_at, last_used_at, revoked_at)
SELECT id, user_id, name, token_hash, prefix, scope, created_at, expires_at, last_used_at, revoked_at
FROM personal_access_tokens;

DROP TABLE personal_access_tokens;
ALTER TABLE personal_access_tokens_v2 RENAME TO personal_access_tokens;

CREATE INDEX IF NOT EXISTS idx_personal_access_tokens_user ON personal_access_tokens (user_id);
