-- What an issue or pull request names (roadmap F17d, V0.2.30): the repository
-- files, symbols and commits its text mentions, resolved to the file artifact
-- they concern. A file's page lists the items that name it. Recomputed when an
-- item is synced and when a repository is.
CREATE TABLE IF NOT EXISTS artifact_mentions (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  from_artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  to_artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  -- file, symbol or commit
  kind TEXT NOT NULL,
  -- the path, symbol name or commit id as the text named it
  label TEXT NOT NULL,
  PRIMARY KEY (from_artifact_id, to_artifact_id, kind, label)
);

CREATE INDEX IF NOT EXISTS idx_artifact_mentions_to ON artifact_mentions (to_artifact_id);
