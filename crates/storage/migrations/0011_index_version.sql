-- Records which version of the indexer produced an artifact's chunks so the
-- server can refresh stale indexes in the background. Artifacts indexed before
-- this migration report 0 and are refreshed once.
ALTER TABLE artifacts ADD COLUMN index_version INTEGER NOT NULL DEFAULT 0;
