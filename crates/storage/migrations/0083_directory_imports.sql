-- Directory migration (V0.8.11): the ledger of files brought in from a Markdown and asset directory. A file is identified by the
-- name of its import (`source_key`) and its path inside the directory, so the same directory sent again changes what changed and
-- makes nothing twice, and an interrupted import carries on from the files already done. `source_hash` is the hash of the file as
-- it was sent (the stored copy may have its links rewritten).
CREATE TABLE IF NOT EXISTS directory_imports (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  source_key TEXT NOT NULL,
  path TEXT NOT NULL,
  artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  source_hash TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, source_key, path)
);

CREATE INDEX IF NOT EXISTS idx_directory_imports_artifact ON directory_imports (artifact_id);
