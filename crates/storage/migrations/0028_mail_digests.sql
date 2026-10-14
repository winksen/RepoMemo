-- The daily digest (roadmap F14b, V0.2.15): when each person was last sent
-- one. A person opts in with the `daily_digest` notification preference.
CREATE TABLE IF NOT EXISTS mail_digests (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  last_sent_at TEXT NOT NULL
);
