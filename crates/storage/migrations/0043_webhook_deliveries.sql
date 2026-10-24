-- The calls a source's webhook received (roadmap F17g, V0.3.22): when, from which sender, for which event,
-- what became of it, and why when it did not become a sync. Only calls that were signed right are kept
-- (a call that cannot be verified says nothing about the source), the newest hundred per source, and
-- they go with the source. A delivery that asked for a sync can be sent again from the page; the row
-- it came from then notes when.
CREATE TABLE IF NOT EXISTS webhook_deliveries (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  received_at TEXT NOT NULL,
  -- github, gitlab, other, or resend for a call made from the page
  sender TEXT NOT NULL,
  event TEXT NOT NULL,
  -- the sender's own id for the call; empty when it gives none
  delivery_id TEXT NOT NULL DEFAULT '',
  -- queued, absorbed (a sync was already running), ignored, pong or failed
  outcome TEXT NOT NULL,
  reason TEXT NOT NULL DEFAULT '',
  resent_at TEXT,
  resent_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_source ON webhook_deliveries (source_id, received_at DESC);
