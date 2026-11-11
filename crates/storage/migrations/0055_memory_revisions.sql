-- Attributed revisions of a memory card's text and citation set (V0.6.3). A revision is immutable: editing or restoring
-- adds the next number and never rewrites an earlier row. `citations_json` is the set as it was, one object per cited
-- passage or file (target type and id, confidence, link metadata, and the title and path shown at the time). A revision
-- goes with its card. Existing cards start at revision 1 with the state they have now and no author.
CREATE TABLE IF NOT EXISTS memory_card_revisions (
  id TEXT PRIMARY KEY,
  card_id TEXT NOT NULL REFERENCES memory_cards(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL,
  title TEXT NOT NULL,
  body_markdown TEXT NOT NULL,
  source TEXT NOT NULL,
  confidence REAL,
  citations_json TEXT NOT NULL DEFAULT '[]',
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  -- the revision number this one restored, when it was made by a restore
  restored_from INTEGER,
  UNIQUE (card_id, revision)
);

INSERT INTO memory_card_revisions (id, card_id, workspace_id, revision, title, body_markdown, source, confidence, citations_json, created_by_user_id, created_at, restored_from)
SELECT lower(hex(randomblob(16))), card.id, card.workspace_id, 1, card.title, card.body_markdown, card.source, card.confidence,
  COALESCE((
    SELECT json_group_array(json_object(
      'target_type', link.to_type, 'target_id', link.to_id, 'confidence', link.confidence,
      'metadata', link.metadata_json, 'title', COALESCE(artifact.title, ''), 'path', COALESCE(artifact.path, '')))
    FROM links AS link
    LEFT JOIN chunks AS chunk ON link.to_type = 'chunk' AND chunk.id = link.to_id
    LEFT JOIN artifacts AS artifact ON artifact.id = CASE WHEN link.to_type = 'artifact' THEN link.to_id ELSE chunk.artifact_id END
    WHERE link.from_type = 'memory_card' AND link.from_id = card.id AND link.relation_type = 'cites'
  ), '[]'),
  NULL, card.updated_at, NULL
FROM memory_cards AS card;
