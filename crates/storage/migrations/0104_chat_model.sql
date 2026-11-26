-- The model a chat uses (V0.9.86): the catalog model a person chose in the chat's picker, kept for the chat so it keeps it from one
-- message to the next. The model that answered each message is already on the run that records it. A chat with none uses what its
-- workspace resolves, and a model that is no longer offered is replaced by that, with a note in the answer.
ALTER TABLE assistant_conversations ADD COLUMN cairn_model_id TEXT;
