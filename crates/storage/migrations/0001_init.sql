PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  settings_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS sources (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  type TEXT NOT NULL,
  name TEXT NOT NULL,
  root_uri TEXT,
  last_indexed_at TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sources_workspace_id ON sources(workspace_id);

CREATE TABLE IF NOT EXISTS blobs (
  hash TEXT PRIMARY KEY,
  storage_uri TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  mime_type TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS artifacts (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  type TEXT NOT NULL,
  title TEXT NOT NULL,
  path TEXT NOT NULL,
  content_hash TEXT NOT NULL REFERENCES blobs(hash),
  mime_type TEXT,
  language TEXT,
  size_bytes INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  indexed_at TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  UNIQUE(workspace_id, source_id, path, content_hash)
);

CREATE INDEX IF NOT EXISTS idx_artifacts_workspace_id ON artifacts(workspace_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_source_id ON artifacts(source_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_path ON artifacts(path);
CREATE INDEX IF NOT EXISTS idx_artifacts_content_hash ON artifacts(content_hash);

CREATE TABLE IF NOT EXISTS chunks (
  id TEXT PRIMARY KEY,
  artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  chunk_index INTEGER NOT NULL,
  text TEXT NOT NULL,
  token_count INTEGER,
  start_line INTEGER,
  end_line INTEGER,
  heading_path TEXT,
  content_hash TEXT NOT NULL,
  embedding_status TEXT NOT NULL DEFAULT 'not_configured',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  UNIQUE(artifact_id, chunk_index)
);

CREATE INDEX IF NOT EXISTS idx_chunks_workspace_id ON chunks(workspace_id);
CREATE INDEX IF NOT EXISTS idx_chunks_artifact_id ON chunks(artifact_id);
CREATE INDEX IF NOT EXISTS idx_chunks_content_hash ON chunks(content_hash);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
  text,
  heading_path,
  artifact_id UNINDEXED,
  workspace_id UNINDEXED,
  content='chunks',
  content_rowid='rowid'
);

CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
  INSERT INTO chunks_fts(rowid, text, heading_path, artifact_id, workspace_id)
  VALUES (new.rowid, new.text, new.heading_path, new.artifact_id, new.workspace_id);
END;

CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text, heading_path, artifact_id, workspace_id)
  VALUES('delete', old.rowid, old.text, old.heading_path, old.artifact_id, old.workspace_id);
END;

CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text, heading_path, artifact_id, workspace_id)
  VALUES('delete', old.rowid, old.text, old.heading_path, old.artifact_id, old.workspace_id);
  INSERT INTO chunks_fts(rowid, text, heading_path, artifact_id, workspace_id)
  VALUES (new.rowid, new.text, new.heading_path, new.artifact_id, new.workspace_id);
END;

CREATE TABLE IF NOT EXISTS symbols (
  id TEXT PRIMARY KEY,
  artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  name TEXT NOT NULL,
  signature TEXT,
  start_line INTEGER,
  end_line INTEGER,
  metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_symbols_workspace_id ON symbols(workspace_id);
CREATE INDEX IF NOT EXISTS idx_symbols_artifact_id ON symbols(artifact_id);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);

CREATE TABLE IF NOT EXISTS links (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  from_id TEXT NOT NULL,
  from_type TEXT NOT NULL,
  to_id TEXT NOT NULL,
  to_type TEXT NOT NULL,
  relation_type TEXT NOT NULL,
  confidence REAL,
  created_by TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_links_workspace_id ON links(workspace_id);
CREATE INDEX IF NOT EXISTS idx_links_from ON links(from_type, from_id);
CREATE INDEX IF NOT EXISTS idx_links_to ON links(to_type, to_id);

CREATE TABLE IF NOT EXISTS memory_cards (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  body_markdown TEXT NOT NULL,
  source TEXT NOT NULL,
  confidence REAL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_memory_cards_workspace_id ON memory_cards(workspace_id);

CREATE TABLE IF NOT EXISTS indexing_jobs (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  source_id TEXT REFERENCES sources(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  stage TEXT NOT NULL,
  progress_current INTEGER NOT NULL DEFAULT 0,
  progress_total INTEGER,
  error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_indexing_jobs_workspace_id ON indexing_jobs(workspace_id);
CREATE INDEX IF NOT EXISTS idx_indexing_jobs_status ON indexing_jobs(status);

CREATE TABLE IF NOT EXISTS provider_settings (
  id TEXT PRIMARY KEY,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
  provider_type TEXT NOT NULL,
  name TEXT NOT NULL,
  base_url TEXT,
  model TEXT,
  embedding_model TEXT,
  enabled INTEGER NOT NULL DEFAULT 0,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_provider_settings_workspace_id ON provider_settings(workspace_id);
