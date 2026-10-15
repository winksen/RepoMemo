-- Items a connector keeps in step with a tracker (roadmap F17a, V0.2.27): one
-- row per issue, and later pull request, that became an artifact. The row
-- tells a sync whether an item is new, changed or unchanged, and what state
-- the tracker last reported (a closed issue becomes *outdated*).
CREATE TABLE IF NOT EXISTS connector_items (
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  state TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  remote_updated_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (source_id, external_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_connector_items_artifact ON connector_items (artifact_id);
