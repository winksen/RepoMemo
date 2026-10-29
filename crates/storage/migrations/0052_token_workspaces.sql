-- A token can be limited to chosen workspaces. A restricted token with no workspace left reaches nothing; a token that
-- is not restricted (every token made before this migration) keeps reaching every workspace its owner can open.
ALTER TABLE personal_access_tokens ADD COLUMN restricted INTEGER NOT NULL DEFAULT 0 CHECK (restricted IN (0,1));

CREATE TABLE personal_token_workspaces (
    token_id TEXT NOT NULL REFERENCES personal_access_tokens(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    PRIMARY KEY (token_id, workspace_id)
);
CREATE INDEX idx_personal_token_workspaces_workspace ON personal_token_workspaces(workspace_id);
