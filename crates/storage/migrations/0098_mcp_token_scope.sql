-- A personal access token may be an MCP token (V0.9.66): it reaches only the MCP endpoint, read-only, for the workspaces it is
-- limited to. It is stored as a read token with this flag, so the table need not be rebuilt, and is shown as the scope `mcp`.
ALTER TABLE personal_access_tokens ADD COLUMN mcp INTEGER NOT NULL DEFAULT 0 CHECK (mcp IN (0,1));
