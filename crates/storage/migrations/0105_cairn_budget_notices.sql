-- Notices of an organization's monthly CAIRN token budget (V0.9.97): one row for each month and threshold (80 or 100 percent), so the
-- organization's administrators are told once at each threshold and not at every run. The usage itself is counted from the stored runs.
CREATE TABLE cairn_budget_notices (
  organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  month TEXT NOT NULL,
  threshold INTEGER NOT NULL CHECK (threshold IN (80, 100)),
  notified_at TEXT NOT NULL,
  PRIMARY KEY (organization_id, month, threshold)
);
