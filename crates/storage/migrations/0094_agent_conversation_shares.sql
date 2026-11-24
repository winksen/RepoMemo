-- A CAIRN chat its owner shares with the workspace (V0.9.57). Readers of the workspace can open it read-only, search it and continue
-- it in a chat of their own; only the owner can confirm what it proposed. Unsharing deletes the row, which ends the access at once,
-- and deleting the chat or the account removes it with the chat.
CREATE TABLE IF NOT EXISTS agent_conversation_shares (
  conversation_id TEXT PRIMARY KEY REFERENCES assistant_conversations(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  shared_by TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  shared_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_conversation_shares_workspace ON agent_conversation_shares(workspace_id, shared_at DESC);
