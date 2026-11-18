-- A connector whose source access was lost or can no longer be established (V0.8.2). While a row is here, the connector's
-- copied evidence is out of search, Ask, the file list and detail, and workspace exports; it is kept (not deleted) until an
-- administrator releases it after the access is back, or removes the connector. `reason` is why, `detail` is the tracker's
-- own words without any secret, and `items` is how many copied items were held back when it began.
CREATE TABLE IF NOT EXISTS connector_quarantine (
  source_id TEXT PRIMARY KEY REFERENCES sources(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  reason TEXT NOT NULL CHECK (reason IN ('access_revoked', 'access_denied', 'not_found')),
  detail TEXT NOT NULL,
  items INTEGER NOT NULL DEFAULT 0,
  quarantined_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_connector_quarantine_workspace ON connector_quarantine (workspace_id);
