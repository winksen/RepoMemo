-- An ability made from a verified playbook keeps which file it came from (V0.9.108), so the ability shows its playbook and the playbook
-- shows its ability. The file is a workspace's, so a deleted file leaves the ability with a link that no longer opens.
ALTER TABLE cairn_abilities ADD COLUMN source_playbook_id TEXT;
CREATE INDEX idx_cairn_abilities_playbook ON cairn_abilities(source_playbook_id) WHERE source_playbook_id IS NOT NULL;
