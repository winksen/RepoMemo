-- Organization profile (V0.8.57): a short description and the key of an icon that stands for the organization. Both are optional and
-- belong to the organization (set by its owner or an administrator, read by its members); the client owns the set of icons, as it does
-- for a workspace.
ALTER TABLE organizations ADD COLUMN description TEXT;
ALTER TABLE organizations ADD COLUMN icon TEXT;
