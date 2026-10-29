-- Who joins a workspace without being asked. `shared` is how every workspace worked before this migration: people who
-- join the organization are added to it, so existing workspaces keep exactly the access they have and gain none.
-- `restricted` never enrolls anyone because of an organization role; its people are added one by one.
ALTER TABLE workspace_organizations
  ADD COLUMN access_policy TEXT NOT NULL DEFAULT 'shared' CHECK (access_policy IN ('shared', 'restricted'));
