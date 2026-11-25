-- The CAIRN inbox (V0.9.67): what an outside agent proposes over MCP waits for a person in a chat of theirs whose origin is `inbox`,
-- one for each person and workspace. Nothing in it is applied until its owner confirms it.
ALTER TABLE assistant_conversations ADD COLUMN origin TEXT NOT NULL DEFAULT 'chat' CHECK (origin IN ('chat', 'inbox'));
