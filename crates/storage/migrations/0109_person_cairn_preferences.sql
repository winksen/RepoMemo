-- A person's CAIRN preferences (V0.9.114): the model they prefer in each workspace, the language and length of answers, whether the
-- steps of a run show open, and Agent or Jobs. They are settings, not memory: read into the person's own runs as a short note, never
-- shared, not in workspace archives, and deleted with the account.
CREATE TABLE person_cairn_preferences (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  preferences_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
