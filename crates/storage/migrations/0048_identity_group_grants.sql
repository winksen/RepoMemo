-- Memberships a provider group created, so a later sync removes only what it granted. A membership that
-- already existed (direct or from the organization) has no row here and is never changed or removed by a sync.
CREATE TABLE identity_group_grants (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('organization','workspace')),
    target_id TEXT NOT NULL,
    role TEXT NOT NULL,
    group_name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, scope, target_id)
);
