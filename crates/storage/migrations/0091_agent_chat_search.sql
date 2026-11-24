-- Search over a person's own CAIRN chats (V0.9.52): the words of each turn's question and of CAIRN's reply, so CAIRN can find what
-- was said in an earlier chat. The index follows the turns: it is filled for the turns that exist, kept current by triggers when
-- a turn is stored, its reply is replaced or it is deleted (a deleted chat takes its turns with it), and a search joins back to
-- the turns and the conversation, so a row of the index alone never returns anything.
CREATE VIRTUAL TABLE IF NOT EXISTS agent_chat_fts USING fts5(
  label,
  reply,
  tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO agent_chat_fts(rowid, label, reply)
  SELECT rowid, label, COALESCE(json_extract(reply_json, '$.reply_markdown'), '') FROM assistant_turns;

CREATE TRIGGER IF NOT EXISTS agent_chat_fts_insert AFTER INSERT ON assistant_turns BEGIN
  INSERT INTO agent_chat_fts(rowid, label, reply)
  VALUES (new.rowid, new.label, COALESCE(json_extract(new.reply_json, '$.reply_markdown'), ''));
END;

CREATE TRIGGER IF NOT EXISTS agent_chat_fts_update AFTER UPDATE OF reply_json, label ON assistant_turns BEGIN
  DELETE FROM agent_chat_fts WHERE rowid = old.rowid;
  INSERT INTO agent_chat_fts(rowid, label, reply)
  VALUES (new.rowid, new.label, COALESCE(json_extract(new.reply_json, '$.reply_markdown'), ''));
END;

CREATE TRIGGER IF NOT EXISTS agent_chat_fts_delete AFTER DELETE ON assistant_turns BEGIN
  DELETE FROM agent_chat_fts WHERE rowid = old.rowid;
END;
