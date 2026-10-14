-- Where each kind of notice reaches a person (roadmap F14a, V0.2.14): in the
-- app, by mail, both or none. No row means both.
CREATE TABLE IF NOT EXISTS notification_preferences (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  channel TEXT NOT NULL CHECK (channel IN ('both', 'app', 'mail', 'none')),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, kind)
);
