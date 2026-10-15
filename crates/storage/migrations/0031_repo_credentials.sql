-- One credential per remote repository link (roadmap S6a, V0.2.19, decision
-- 11): a read-only token for https or a deploy key for ssh. The secret is
-- sealed like provider keys and is never returned by the API or logged;
-- `username` is the account name that goes with a token and is not secret.
CREATE TABLE IF NOT EXISTS repo_credentials (
  source_id TEXT PRIMARY KEY REFERENCES sources(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('token', 'deploy_key')),
  username TEXT,
  sealed_secret TEXT NOT NULL,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
