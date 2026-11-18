-- Public snapshots (V0.8.16): a publication is a COPY of text a publisher previewed and approved. It holds only that text, a title and
-- an opaque public address (`slug`). It has no link to people, comments, attachments or paths. `source_artifact_id` is kept for the
-- publisher's own screens and for revocation (V0.8.17); it is never part of anything public. The private original can change or go
-- without changing the snapshot.
CREATE TABLE IF NOT EXISTS publications (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  slug TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  body_markdown TEXT NOT NULL,
  snapshot_hash TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'published' CHECK (status IN ('published', 'withdrawn', 'stale')),
  version INTEGER NOT NULL DEFAULT 1,
  source_artifact_id TEXT REFERENCES artifacts(id) ON DELETE SET NULL,
  published_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  published_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_publications_workspace ON publications (workspace_id, status);
CREATE INDEX IF NOT EXISTS idx_publications_source ON publications (source_artifact_id);
