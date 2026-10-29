-- A guest (a person in a workspace who is not in its organization) can be given an end date. Past it the membership
-- grants nothing: it stays on record so it can be renewed, and `expiry_noted_at` marks that the end was written to
-- the audit trail. Everyone who already has a membership keeps it with no end date.
ALTER TABLE workspace_memberships ADD COLUMN expires_at TEXT;
ALTER TABLE workspace_memberships ADD COLUMN expiry_noted_at TEXT;
