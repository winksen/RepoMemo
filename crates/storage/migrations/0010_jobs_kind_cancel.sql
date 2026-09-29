-- Generalizes indexing_jobs into a jobs table with an explicit kind and
-- a cooperative cancellation flag. Existing rows are treated as `indexing`
-- jobs; new kinds ("embedding", "connector_sync", "export", ...) reuse the
-- same table so the shared server can expose one jobs API to the client.

ALTER TABLE indexing_jobs ADD COLUMN kind TEXT NOT NULL DEFAULT 'indexing';
ALTER TABLE indexing_jobs ADD COLUMN cancel_requested INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_indexing_jobs_kind ON indexing_jobs(kind);
CREATE INDEX IF NOT EXISTS idx_indexing_jobs_workspace_status
  ON indexing_jobs(workspace_id, status);
