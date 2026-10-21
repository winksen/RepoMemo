-- A person's role in a workspace now remembers where it came from (review E9). `organization`
-- rows follow the organization role of their holder; `direct` rows were set by a workspace
-- owner or administrator and are left alone when an organization role changes. Rows made
-- before this migration are `direct`, unless they match the organization role exactly (then
-- they were most likely made by it). A workspace administrator who removes a member of the
-- organization is remembered in `workspace_membership_removals`, so the next organization
-- role change does not bring that person back.
ALTER TABLE workspace_memberships
  ADD COLUMN origin TEXT NOT NULL DEFAULT 'direct' CHECK (origin IN ('organization', 'direct'));

UPDATE workspace_memberships SET origin = 'organization'
WHERE EXISTS (
  SELECT 1
  FROM workspace_organizations wo
  JOIN organization_memberships om ON om.organization_id = wo.organization_id
  WHERE wo.workspace_id = workspace_memberships.workspace_id
    AND om.user_id = workspace_memberships.user_id
    AND workspace_memberships.role = CASE om.role WHEN 'owner' THEN 'owner' WHEN 'admin' THEN 'admin' ELSE 'member' END
);

CREATE TABLE workspace_membership_removals (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  removed_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, user_id)
);
