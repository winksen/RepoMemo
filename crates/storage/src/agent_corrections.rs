//! What a person told CAIRN was wrong (V0.9.53), and whether they stopped being offered to save decisions in a chat.

use super::*;

/// The most characters of a correction.
pub const CORRECTION_CHARS: usize = 400;
/// The most open corrections a person keeps in one workspace; the oldest are dropped past it.
pub const CORRECTIONS_OPEN_MAX: i64 = 20;

fn clean_correction(text: &str) -> Result<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        repomemo_domain::bail_invalid!("Say what was wrong.");
    }
    Ok(text.chars().take(CORRECTION_CHARS).collect())
}

impl StorageEngine {
    /// Records a correction against a turn, open and unreviewed. Past the bound the oldest open ones are dropped.
    pub async fn add_agent_correction(
        &self,
        workspace_id: &str,
        user_id: &str,
        conversation_id: Option<&str>,
        turn_id: Option<&str>,
        text: &str,
    ) -> Result<String> {
        let text = clean_correction(text)?;
        let id = Uuid::new_v4().to_string();
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO agent_corrections (id, workspace_id, user_id, conversation_id, turn_id, text, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7)",
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(user_id)
        .bind(conversation_id)
        .bind(turn_id)
        .bind(&text)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "DELETE FROM agent_corrections WHERE workspace_id = ?1 AND user_id = ?2 AND state = 'open' AND id NOT IN (SELECT id FROM agent_corrections WHERE workspace_id = ?1 AND user_id = ?2 AND state = 'open' ORDER BY created_at DESC, id LIMIT ?3)",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(CORRECTIONS_OPEN_MAX)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(id)
    }

    /// The person's open corrections in a workspace, newest first, as the lines a later run is given.
    pub async fn open_agent_corrections(&self, workspace_id: &str, user_id: &str, limit: i64) -> Result<Vec<String>> {
        Ok(sqlx::query_scalar(
            "SELECT text FROM agent_corrections WHERE workspace_id = ?1 AND user_id = ?2 AND state = 'open' ORDER BY created_at DESC, id LIMIT ?3",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(limit)
        .fetch_all(self.reader())
        .await?)
    }

    /// Settles every open correction recorded against a turn, as when its memory proposal was confirmed.
    pub async fn settle_agent_corrections(&self, turn_id: &str, state: &str) -> Result<()> {
        sqlx::query("UPDATE agent_corrections SET state = ?2 WHERE turn_id = ?1 AND state = 'open'")
            .bind(turn_id)
            .bind(state)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Stops offering to save decisions in a chat.
    pub async fn stop_decision_offers(&self, conversation_id: &str) -> Result<()> {
        sqlx::query("UPDATE assistant_conversations SET decision_offers_off = 1 WHERE id = ?1")
            .bind(conversation_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Whether a chat has stopped being offered to save decisions.
    pub async fn decision_offers_off(&self, conversation_id: &str) -> Result<bool> {
        let off: Option<i64> =
            sqlx::query_scalar("SELECT decision_offers_off FROM assistant_conversations WHERE id = ?1")
                .bind(conversation_id)
                .fetch_optional(self.reader())
                .await?;
        Ok(off.is_some_and(|value| value != 0))
    }
}

#[cfg(test)]
mod tests {
    use super::clean_correction;

    #[test]
    fn a_correction_is_one_bounded_line() {
        assert_eq!(clean_correction("  The limit is\n50 ").unwrap(), "The limit is 50");
        assert!(clean_correction("  ").is_err());
        assert_eq!(clean_correction(&"a".repeat(500)).unwrap().chars().count(), 400);
    }
}
