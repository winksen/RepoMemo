-- Answer corrections (V0.8.14): search feedback gains a third kind, `wrong_answer`, for an answer that was wrong, out of date or
-- built on a source that should not be trusted. SQLite cannot widen a CHECK, so the table is rebuilt with the same columns and rows.
CREATE TABLE IF NOT EXISTS search_feedback_v2 (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK (kind IN ('no_answer', 'unhelpful_result', 'wrong_answer')),
  query TEXT NOT NULL,
  artifact_id TEXT REFERENCES artifacts(id) ON DELETE SET NULL,
  note TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved', 'declined')),
  resolution_kind TEXT CHECK (resolution_kind IN ('evidence', 'memory_card')),
  resolution_id TEXT,
  resolved_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  resolved_note TEXT NOT NULL DEFAULT '',
  resolved_at TEXT,
  created_at TEXT NOT NULL
);

INSERT INTO search_feedback_v2
  (id, workspace_id, user_id, kind, query, artifact_id, note, status, resolution_kind, resolution_id, resolved_by_user_id,
   resolved_note, resolved_at, created_at)
SELECT id, workspace_id, user_id, kind, query, artifact_id, note, status, resolution_kind, resolution_id, resolved_by_user_id,
       resolved_note, resolved_at, created_at
FROM search_feedback;

DROP TABLE search_feedback;
ALTER TABLE search_feedback_v2 RENAME TO search_feedback;

CREATE INDEX IF NOT EXISTS idx_search_feedback_workspace ON search_feedback (workspace_id, status, created_at);
CREATE INDEX IF NOT EXISTS idx_search_feedback_user ON search_feedback (user_id, workspace_id);
