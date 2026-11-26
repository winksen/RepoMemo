-- CAIRN abilities (V0.9.104): procedures an organization or a workspace teaches CAIRN, run with a /command. An ability is a name, a
-- command, a description and, in each version, instructions, the tools it may use (a subset of the registry), optional input fields and an
-- output template. It never adds a tool, an external effect or a right the person lacks. Every version is kept.
CREATE TABLE cairn_abilities (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL CHECK (scope IN ('organization', 'workspace')),
  organization_id TEXT REFERENCES organizations(id) ON DELETE CASCADE,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  command TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  current_version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
  CHECK ((scope = 'organization' AND organization_id IS NOT NULL AND workspace_id IS NULL)
      OR (scope = 'workspace' AND workspace_id IS NOT NULL AND organization_id IS NULL))
);
CREATE UNIQUE INDEX idx_cairn_abilities_org_command ON cairn_abilities(organization_id, command) WHERE organization_id IS NOT NULL;
CREATE UNIQUE INDEX idx_cairn_abilities_workspace_command ON cairn_abilities(workspace_id, command) WHERE workspace_id IS NOT NULL;

CREATE TABLE cairn_ability_versions (
  ability_id TEXT NOT NULL REFERENCES cairn_abilities(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  instructions TEXT NOT NULL,
  tools_json TEXT NOT NULL DEFAULT '[]',
  inputs_json TEXT NOT NULL DEFAULT '[]',
  output_template TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
  PRIMARY KEY (ability_id, version)
);
