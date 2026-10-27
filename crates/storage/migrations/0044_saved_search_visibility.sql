-- Existing saved searches were workspace-wide. New searches are private until shared.
ALTER TABLE workspace_saved_searches ADD COLUMN shared INTEGER NOT NULL DEFAULT 1 CHECK (shared IN (0, 1));
CREATE INDEX IF NOT EXISTS idx_saved_search_visibility
  ON workspace_saved_searches(workspace_id, shared, created_by_user_id, updated_at DESC);
