-- Abilities an organization teaches are on in its workspaces until a workspace's administrators turn one off (V0.9.106). Only the
-- ones turned off are kept; deleting the ability or the workspace removes the row.
CREATE TABLE workspace_ability_off (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  ability_id TEXT NOT NULL REFERENCES cairn_abilities(id) ON DELETE CASCADE,
  turned_off_at TEXT NOT NULL,
  turned_off_by TEXT REFERENCES users(id) ON DELETE SET NULL,
  PRIMARY KEY (workspace_id, ability_id)
);
