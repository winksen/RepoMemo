-- Ways into an account that do not need a password typed in a browser.
-- Every secret is stored as its SHA-256 only; the raw value is shown once.

-- Invitation links to an organization or a workspace, for people who may not
-- have an account yet. Accepting one applies the same membership rules as
-- adding the person by email. `target_id` is polymorphic (no foreign key); an
-- invitation to a deleted target simply cannot be accepted.
CREATE TABLE IF NOT EXISTS invitations (
  id TEXT PRIMARY KEY,
  token_hash TEXT NOT NULL UNIQUE,
  email TEXT NOT NULL,
  target_type TEXT NOT NULL CHECK (target_type IN ('organization', 'workspace')),
  target_id TEXT NOT NULL,
  role TEXT NOT NULL,
  invited_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  accepted_at TEXT,
  accepted_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_invitations_target ON invitations (target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_invitations_expires ON invitations (expires_at);

-- One-time password reset links, issued by a system administrator.
CREATE TABLE IF NOT EXISTS password_resets (
  token_hash TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_password_resets_user ON password_resets (user_id);
CREATE INDEX IF NOT EXISTS idx_password_resets_expires ON password_resets (expires_at);

-- Personal access tokens for scripts, CI and the terminal console. They act
-- as their user, read-only or with full rights, until they expire or are
-- revoked. `prefix` is the start of the token, shown to tell tokens apart.
CREATE TABLE IF NOT EXISTS personal_access_tokens (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  prefix TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('read', 'write')),
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  last_used_at TEXT,
  revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_personal_access_tokens_user ON personal_access_tokens (user_id);
