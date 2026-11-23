-- A rolling summary of the turns of a CAIRN chat that no longer fit the model's window (V0.9.48): its text, the position of the
-- last turn it covers and the files those turns were about. It is the person's own, like the chat, and goes with it.
ALTER TABLE assistant_conversations ADD COLUMN summary_json TEXT;
