-- The model catalog (V0.9.75): the connections to model servers and the models they offer, kept once for the whole server
-- instead of once for each workspace. A connection holds the address and the encrypted key; a model holds what CAIRN needs to
-- choose it: its purpose, its traits, its evaluation, its fallback and the roles allowed to use it. Nothing reads these tables yet.
-- The key is sealed with the server's secret key as provider keys are, and is never returned by a read.
CREATE TABLE cairn_connections (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('ollama', 'openrouter', 'openai_compatible')),
  name TEXT NOT NULL,
  base_url TEXT NOT NULL,
  api_key_sealed TEXT,
  locality TEXT NOT NULL CHECK (locality IN ('local', 'cloud')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  cloud_acknowledged_at TEXT,
  cloud_acknowledged_by TEXT REFERENCES users(id) ON DELETE SET NULL,
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  updated_by TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE cairn_models (
  id TEXT PRIMARY KEY,
  connection_id TEXT NOT NULL REFERENCES cairn_connections(id) ON DELETE RESTRICT,
  model_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  purpose TEXT NOT NULL CHECK (purpose IN ('text', 'vision', 'embedding')),
  tool_mode TEXT NOT NULL DEFAULT 'none' CHECK (tool_mode IN ('none', 'json', 'native')),
  context_tokens INTEGER NOT NULL DEFAULT 0,
  vision INTEGER NOT NULL DEFAULT 0 CHECK (vision IN (0, 1)),
  evaluation_json TEXT,
  fallback_model_id TEXT REFERENCES cairn_models(id) ON DELETE SET NULL,
  roles_json TEXT,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  updated_by TEXT REFERENCES users(id) ON DELETE SET NULL,
  UNIQUE (connection_id, model_id, purpose)
);

CREATE INDEX idx_cairn_models_connection ON cairn_models(connection_id);
