-- The file list of a workspace is paged in SQL, newest change first (review P15a). This index holds
-- that order, so a page is found by reading the first rows of it and not by sorting every file of
-- the workspace.
CREATE INDEX IF NOT EXISTS idx_artifacts_workspace_updated ON artifacts(workspace_id, updated_at DESC, path ASC);
