-- Workspace assistant chats. A conversation belongs to one user in one
-- workspace and is visible only to that user. Each turn stores the request
-- and the assistant's reply together, so a failed request leaves no row.
CREATE TABLE IF NOT EXISTS assistant_conversations (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_assistant_conversations_owner
  ON assistant_conversations(workspace_id, user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS assistant_turns (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES assistant_conversations(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  label TEXT NOT NULL,
  request_json TEXT NOT NULL,
  reply_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assistant_turns_position
  ON assistant_turns(conversation_id, position);
