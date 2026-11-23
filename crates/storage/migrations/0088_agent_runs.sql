-- CAIRN runs and their traces (V0.9.39). A run is one message answered: the tool it ran or, from the loop, the
-- tools it chained, with a one-line summary of each step, how long it took and the tokens it used. A trace is
-- personal like the conversation it belongs to: it is deleted with the chat and with the account's anonymization,
-- is not part of a workspace archive and is kept only for the retention period. Steps hold summaries and the
-- files they named, never the text of a passage.
CREATE TABLE IF NOT EXISTS agent_runs (
  id TEXT PRIMARY KEY,
  turn_id TEXT REFERENCES assistant_turns(id) ON DELETE CASCADE,
  conversation_id TEXT NOT NULL REFERENCES assistant_conversations(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  origin TEXT NOT NULL CHECK (origin IN ('chat', 'routine', 'inbox')),
  provider_id TEXT,
  model TEXT,
  tool_mode TEXT,
  state TEXT NOT NULL CHECK (state IN ('running', 'answered', 'awaiting_confirmation', 'partial', 'failed', 'cancelled')),
  steps INTEGER NOT NULL DEFAULT 0,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  started_at TEXT NOT NULL,
  ended_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_agent_runs_turn ON agent_runs(turn_id);
CREATE INDEX IF NOT EXISTS idx_agent_runs_started ON agent_runs(started_at);

CREATE TABLE IF NOT EXISTS agent_steps (
  run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('model', 'tool', 'proposal', 'answer', 'note')),
  tool TEXT,
  arguments_json TEXT,
  summary TEXT NOT NULL,
  evidence_json TEXT NOT NULL DEFAULT '[]',
  duration_ms INTEGER NOT NULL DEFAULT 0,
  error TEXT,
  PRIMARY KEY (run_id, position)
);
