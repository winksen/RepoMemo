-- Earlier versions of an uploaded file (roadmap F24, V0.2.38). Uploading a new
-- version replaces the artifact's content in place; the content it had until
-- then is kept here, so its text stays readable and can be compared. The
-- current content is the artifact's own; version numbers run from 1 (the first
-- upload) and the artifact holds the highest.
CREATE TABLE IF NOT EXISTS artifact_versions (
  id TEXT PRIMARY KEY,
  artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  content_hash TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  mime_type TEXT,
  -- when this content became the file's current content
  became_current_at TEXT NOT NULL,
  -- when it was replaced, and by whom (the person who uploaded the next version)
  replaced_at TEXT NOT NULL,
  replaced_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  UNIQUE (artifact_id, version)
);

-- A blob an earlier version holds is not collected as unreferenced.
CREATE INDEX IF NOT EXISTS idx_artifact_versions_hash ON artifact_versions (content_hash);
