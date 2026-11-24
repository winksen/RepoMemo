-- A CAIRN chat may cover all of a person's workspaces (V0.9.60). The chat still belongs to the workspace it was started in, but its
-- lookups fan out to every workspace the person can read, each result labelled with its own workspace.
ALTER TABLE assistant_conversations ADD COLUMN all_workspaces INTEGER NOT NULL DEFAULT 0;
