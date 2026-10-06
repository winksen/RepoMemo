-- Files of git repository sources. Each tracked path maps to one artifact that
-- is updated in place as the repository changes, so comments, memory links and
-- lifecycle history survive edits and renames. A file that leaves the tree is
-- kept with `removed_at` set (and its chunks dropped) instead of being deleted,
-- because memory cards and comments may still point at it.
CREATE TABLE IF NOT EXISTS repo_files (
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  blob_sha TEXT NOT NULL,
  commit_sha TEXT NOT NULL,
  removed_at TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (source_id, path)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_repo_files_artifact ON repo_files (artifact_id);
CREATE INDEX IF NOT EXISTS idx_repo_files_source_blob ON repo_files (source_id, blob_sha);
