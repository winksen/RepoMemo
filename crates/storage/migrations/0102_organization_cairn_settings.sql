-- An organization's CAIRN settings (V0.9.82): which catalog models it uses, its default model and its Agent-mode default, kept as one
-- versioned policy so two administrators cannot overwrite each other unseen. A row exists only once someone saved it; an organization
-- with none uses the defaults of the code. The policy is not in workspace archives. `updated_by` keeps the person, who reads as
-- *Former member* once their account is anonymized.
CREATE TABLE organization_cairn_settings (
  organization_id TEXT PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
  policy_json TEXT NOT NULL DEFAULT '{}',
  version INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL,
  updated_by TEXT REFERENCES users(id) ON DELETE SET NULL
);
