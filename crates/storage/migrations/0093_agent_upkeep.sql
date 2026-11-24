-- Memory upkeep (V0.9.56): an opt-in weekly pass over a workspace's memory that finds what needs a person's attention. It is off for
-- every workspace until an administrator turns it on. A finding is a note for the review queue with the evidence it rests on;
-- the pass edits nothing.
CREATE TABLE IF NOT EXISTS agent_upkeep_settings (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL DEFAULT 0,
  last_run_at TEXT
);

CREATE TABLE IF NOT EXISTS agent_upkeep_findings (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  -- possible_contradiction, unanswered_question or unused_playbook
  kind TEXT NOT NULL,
  -- Stable for the same facts, so a finding that was resolved is not raised again.
  fingerprint TEXT NOT NULL,
  title TEXT NOT NULL,
  detail TEXT NOT NULL,
  evidence TEXT NOT NULL DEFAULT '',
  artifact_id TEXT,
  state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'resolved')),
  created_at TEXT NOT NULL,
  UNIQUE (workspace_id, fingerprint)
);
CREATE INDEX IF NOT EXISTS idx_agent_upkeep_findings_open ON agent_upkeep_findings(workspace_id, state, created_at);
