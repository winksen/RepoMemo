-- Opt-in signed outbound webhooks (V0.8.4). An endpoint is a destination an administrator approved: its address must be on the
-- operator's list, its signing secret is sealed, and it names the events it receives. A delivery is one event for one endpoint: the
-- event id is the same for every endpoint and every retry, so a receiver can tell a repeat from a new event. A disabled or deleted
-- endpoint receives nothing that was still queued (those deliveries become `cancelled`).
CREATE TABLE IF NOT EXISTS outbound_endpoints (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  url TEXT NOT NULL,
  sealed_secret TEXT NOT NULL,
  events_json TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_delivered_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_outbound_endpoints_workspace ON outbound_endpoints (workspace_id);

CREATE TABLE IF NOT EXISTS outbound_deliveries (
  id TEXT PRIMARY KEY,
  endpoint_id TEXT NOT NULL REFERENCES outbound_endpoints(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  event_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'delivered', 'failed', 'cancelled')),
  attempts INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT,
  last_status_code INTEGER,
  last_error TEXT,
  created_at TEXT NOT NULL,
  delivered_at TEXT,
  UNIQUE (endpoint_id, event_id)
);

CREATE INDEX IF NOT EXISTS idx_outbound_deliveries_due ON outbound_deliveries (status, next_attempt_at);
CREATE INDEX IF NOT EXISTS idx_outbound_deliveries_endpoint ON outbound_deliveries (endpoint_id, created_at DESC);
