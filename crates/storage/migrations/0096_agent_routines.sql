-- Routines (V0.9.63): a prompt a person saves for CAIRN to run on a schedule, as themselves, with read tools only. Each run is posted
-- into one chat named after the routine and tells the owner. A routine pauses by itself when its owner can no longer change the
-- workspace, when the workspace has no AI provider, or after three failed runs in a row.
CREATE TABLE IF NOT EXISTS agent_routines (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  prompt TEXT NOT NULL,
  -- A RecurringSchedule as JSON: daily, weekly or monthly, as for recurring tasks.
  schedule_json TEXT NOT NULL,
  -- The date (YYYY-MM-DD, UTC) of the next run; the run happens in the first maintenance pass on or after it.
  next_run_date TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'paused')),
  pause_reason TEXT,
  failures INTEGER NOT NULL DEFAULT 0,
  last_run_at TEXT,
  conversation_id TEXT REFERENCES assistant_conversations(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_routines_workspace ON agent_routines(workspace_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agent_routines_due ON agent_routines(state, next_run_date);
