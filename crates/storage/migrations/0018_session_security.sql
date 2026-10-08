-- Bumped whenever all of a user's sessions must end (password change, "sign out
-- everywhere"). Access tokens carry the version they were issued with and are
-- refused once it no longer matches, so they stop working before they expire.
ALTER TABLE users ADD COLUMN session_version INTEGER NOT NULL DEFAULT 0;

-- Expired refresh tokens are purged by the background maintenance pass.
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_expires_at ON refresh_tokens (expires_at);

-- Blob garbage collection looks for blobs no artifact references any more.
CREATE INDEX IF NOT EXISTS idx_blobs_created_at ON blobs (created_at);

-- Finished jobs older than the retention period are pruned.
CREATE INDEX IF NOT EXISTS idx_indexing_jobs_status_updated ON indexing_jobs (status, updated_at);
