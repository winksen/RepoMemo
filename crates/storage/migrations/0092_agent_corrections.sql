-- What a person told CAIRN was wrong (V0.9.53). A correction is the person's own statement, kept against the turn it is about, and
-- is read into that person's later runs in the same workspace as a note nobody has reviewed. Nothing here is evidence or a
-- memory card: a card is only made when the person confirms CAIRN's proposal.
CREATE TABLE IF NOT EXISTS agent_corrections (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  conversation_id TEXT REFERENCES assistant_conversations(id) ON DELETE CASCADE,
  turn_id TEXT REFERENCES assistant_turns(id) ON DELETE SET NULL,
  text TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'reviewed', 'dismissed')),
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_corrections_owner
  ON agent_corrections(workspace_id, user_id, state, created_at DESC);

-- A person who dismissed the offer to save a decision stops being offered it in that chat.
ALTER TABLE assistant_conversations ADD COLUMN decision_offers_off INTEGER NOT NULL DEFAULT 0;
