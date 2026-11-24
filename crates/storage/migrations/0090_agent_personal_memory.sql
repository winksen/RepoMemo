-- What CAIRN remembers about one person (V0.9.51): preferences and facts such as "answers in French" or "works on the billing
-- project". CAIRN proposes an entry and the person confirms it; the person lists, edits and deletes their entries in their
-- profile and can turn the memory off. An entry belongs to the person alone: it is read into that person's own runs only, is
-- removed when the account is anonymized, and is never part of a workspace archive.
CREATE TABLE IF NOT EXISTS agent_personal_memory (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
  text TEXT NOT NULL,
  source_turn_id TEXT REFERENCES assistant_turns(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_personal_memory_user ON agent_personal_memory(user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS agent_personal_memory_settings (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL DEFAULT 1
);
