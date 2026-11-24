//! Search over a person's own CAIRN chats (V0.9.52). Only the caller's conversations are searched, in one workspace, and a chat
//! that was deleted is gone from the results because the search joins back to the turns and the conversation.

use super::*;

/// One earlier exchange that matched.
#[derive(Debug, Clone)]
pub struct ChatHit {
    pub turn_id: String,
    pub conversation_id: String,
    pub conversation_title: String,
    pub workspace_id: String,
    /// What the person asked.
    pub label: String,
    /// The part of CAIRN's reply that matched.
    pub snippet: String,
    pub created_at: String,
}

impl StorageEngine {
    /// The caller's exchanges in one workspace that match an FTS query, best first. Another person's chat never matches.
    pub async fn search_agent_chats(
        &self,
        user_id: &str,
        workspace_id: &str,
        fts_query: &str,
        limit: i64,
    ) -> Result<Vec<ChatHit>> {
        let rows = sqlx::query(
            "SELECT turn.id AS turn_id, turn.conversation_id, conversation.title, conversation.workspace_id, turn.label, \
                    snippet(agent_chat_fts, 1, '', '', ' … ', 28) AS snippet, turn.created_at \
             FROM agent_chat_fts \
             JOIN assistant_turns turn ON turn.rowid = agent_chat_fts.rowid \
             JOIN assistant_conversations conversation ON conversation.id = turn.conversation_id \
             WHERE agent_chat_fts MATCH ?1 AND conversation.user_id = ?2 AND conversation.workspace_id = ?3 \
             ORDER BY rank LIMIT ?4",
        )
        .bind(fts_query)
        .bind(user_id)
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| ChatHit {
                turn_id: row.get("turn_id"),
                conversation_id: row.get("conversation_id"),
                conversation_title: row.get("title"),
                workspace_id: row.get("workspace_id"),
                label: row.get("label"),
                snippet: row.get("snippet"),
                created_at: row.get("created_at"),
            })
            .collect())
    }
}
