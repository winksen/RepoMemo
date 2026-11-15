-- Search feedback (V0.6.25): a question the evidence could not answer, or a result that did not help, sent to the people who
-- can improve the evidence. The query text is kept only as long as it is useful: an open request until it is resolved (at
-- most 180 days), a resolved or declined one for 30 days. Each is removed after that, and with its workspace.
CREATE TABLE IF NOT EXISTS search_feedback (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  -- `no_answer` (nothing found or answered) or `unhelpful_result` (a result that did not help)
  kind TEXT NOT NULL CHECK (kind IN ('no_answer', 'unhelpful_result')),
  query TEXT NOT NULL,
  artifact_id TEXT REFERENCES artifacts(id) ON DELETE SET NULL,
  note TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved', 'declined')),
  -- what resolved it: new or corrected evidence, or a memory card
  resolution_kind TEXT CHECK (resolution_kind IN ('evidence', 'memory_card')),
  resolution_id TEXT,
  resolved_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  resolved_note TEXT NOT NULL DEFAULT '',
  resolved_at TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_search_feedback_workspace ON search_feedback (workspace_id, status, created_at);
CREATE INDEX IF NOT EXISTS idx_search_feedback_user ON search_feedback (user_id, workspace_id);
