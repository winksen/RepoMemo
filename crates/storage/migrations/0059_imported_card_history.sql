-- Revision 1 for every memory card that has no revisions (V0.6.12). Cards made by a workspace import before this release
-- were inserted without one, so they reported revision 0, could be approved at it, and their first edit became revision 1
-- and left the imported text out of the history. Only a card with no revision gets one; a card with history is untouched.
-- The cited content is left unrecorded (the card shows "no recorded content"): what the import cited cannot be told from
-- the file as it is now.
INSERT INTO memory_card_revisions (id, card_id, workspace_id, revision, title, body_markdown, source, confidence, citations_json, created_by_user_id, created_at, restored_from)
SELECT lower(hex(randomblob(16))), card.id, card.workspace_id, 1, card.title, card.body_markdown, card.source, card.confidence,
  COALESCE((
    SELECT json_group_array(json_object(
      'target_type', link.to_type, 'target_id', link.to_id, 'confidence', link.confidence,
      'metadata', link.metadata_json, 'title', COALESCE(artifact.title, ''), 'path', COALESCE(artifact.path, ''),
      'artifact_id', COALESCE(artifact.id, ''), 'content_hash', ''))
    FROM links AS link
    LEFT JOIN chunks AS chunk ON link.to_type = 'chunk' AND chunk.id = link.to_id
    LEFT JOIN artifacts AS artifact ON artifact.id = CASE WHEN link.to_type = 'artifact' THEN link.to_id ELSE chunk.artifact_id END
    WHERE link.from_type = 'memory_card' AND link.from_id = card.id AND link.relation_type = 'cites'
  ), '[]'),
  NULL, card.updated_at, NULL
FROM memory_cards AS card
WHERE NOT EXISTS (SELECT 1 FROM memory_card_revisions AS kept WHERE kept.card_id = card.id);
