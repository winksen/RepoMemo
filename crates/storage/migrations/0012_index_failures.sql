-- Terminal indexing failures, kept so the shared client can show why a file is
-- not searchable. A row is removed when the artifact is indexed successfully.
CREATE TABLE IF NOT EXISTS artifact_index_failures (
  artifact_id TEXT PRIMARY KEY,
  message TEXT NOT NULL,
  attempts INTEGER NOT NULL,
  failed_at TEXT NOT NULL
);
