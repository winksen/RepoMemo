-- Organization templates (V0.6.23): templates the administrators of an organization saved, held to the same rules as the
-- files of the built-in and operator folders. Each save is a new row with the next version, so earlier versions stay
-- readable and nothing is rewritten. A template id belongs to the first organization that saved it. A template only
-- arranges and names what a workspace shows; it never carries a right and never changes or moves content.
CREATE TABLE IF NOT EXISTS organization_templates (
  id TEXT PRIMARY KEY,
  organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  template_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  name TEXT NOT NULL,
  summary TEXT NOT NULL,
  definition_json TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  UNIQUE (organization_id, template_id, version)
);

CREATE INDEX IF NOT EXISTS idx_organization_templates_template ON organization_templates (template_id, version);
