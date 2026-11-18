//! Chat destinations and their digest ledger (V0.8.9).

use super::*;
use repomemo_domain::ChatDestination;

/// The digests a destination may ask for.
pub const CHAT_DIGESTS: [&str; 2] = ["review", "projects"];
const MAX_DESTINATIONS_PER_WORKSPACE: i64 = 10;

fn url_context(id: &str) -> String {
    format!("chat_destinations:{id}:url")
}

type Row = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
);

const COLUMNS: &str =
    "id, workspace_id, provider, name, host, digests_json, enabled, created_at, updated_at, last_sent_at, last_error";

fn destination_of(row: Row) -> ChatDestination {
    ChatDestination {
        id: row.0,
        workspace_id: row.1,
        provider: row.2,
        name: row.3,
        host: row.4,
        digests: serde_json::from_str(&row.5).unwrap_or_default(),
        enabled: row.6 != 0,
        created_at: row.7,
        last_sent_at: row.9,
        last_error: row.10,
    }
}

/// What a destination is made from.
pub struct NewChatDestination<'a> {
    pub provider: &'a str,
    pub name: &'a str,
    /// The address to post to: a secret.
    pub url: &'a str,
    /// Where it is, without the secret path.
    pub host: &'a str,
    pub digests: &'a [String],
}

impl StorageEngine {
    pub async fn create_chat_destination(
        &self,
        workspace_id: &str,
        new: NewChatDestination<'_>,
        actor_user_id: &str,
    ) -> Result<ChatDestination> {
        let held: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_destinations WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(self.reader())
            .await?;
        if held >= MAX_DESTINATIONS_PER_WORKSPACE {
            repomemo_domain::bail_conflict!(
                "A workspace can have {MAX_DESTINATIONS_PER_WORKSPACE} chat destinations. Remove one first."
            );
        }
        let id = Uuid::new_v4().to_string();
        let sealed = self.secrets.seal(new.url, &url_context(&id))?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO chat_destinations
               (id, workspace_id, provider, name, host, sealed_url, digests_json, enabled, created_by_user_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?9)"#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(new.provider)
        .bind(new.name)
        .bind(new.host)
        .bind(sealed)
        .bind(serde_json::to_string(new.digests)?)
        .bind(actor_user_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.chat_destination(&id).await
    }

    pub async fn chat_destination(&self, id: &str) -> Result<ChatDestination> {
        let row = sqlx::query_as::<_, Row>(&format!("SELECT {COLUMNS} FROM chat_destinations WHERE id = ?1"))
            .bind(id)
            .fetch_optional(self.reader())
            .await?;
        match row {
            Some(row) => Ok(destination_of(row)),
            None => repomemo_domain::bail_not_found!("Destination was not found."),
        }
    }

    pub async fn list_chat_destinations(&self, workspace_id: &str) -> Result<Vec<ChatDestination>> {
        Ok(sqlx::query_as::<_, Row>(&format!(
            "SELECT {COLUMNS} FROM chat_destinations WHERE workspace_id = ?1 ORDER BY created_at ASC"
        ))
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(destination_of)
        .collect())
    }

    /// Every destination that is on, for the digest pass.
    pub async fn list_enabled_chat_destinations(&self) -> Result<Vec<ChatDestination>> {
        Ok(sqlx::query_as::<_, Row>(&format!(
            "SELECT {COLUMNS} FROM chat_destinations WHERE enabled = 1 ORDER BY created_at ASC"
        ))
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(destination_of)
        .collect())
    }

    pub async fn update_chat_destination(
        &self,
        id: &str,
        name: Option<&str>,
        digests: Option<&[String]>,
        enabled: Option<bool>,
    ) -> Result<ChatDestination> {
        let current = self.chat_destination(id).await?;
        sqlx::query(
            "UPDATE chat_destinations SET name = ?2, digests_json = ?3, enabled = ?4, updated_at = ?5 WHERE id = ?1",
        )
        .bind(id)
        .bind(name.unwrap_or(&current.name))
        .bind(serde_json::to_string(digests.unwrap_or(&current.digests))?)
        .bind(i64::from(enabled.unwrap_or(current.enabled)))
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        self.chat_destination(id).await
    }

    pub async fn delete_chat_destination(&self, id: &str) -> Result<()> {
        self.chat_destination(id).await?;
        sqlx::query("DELETE FROM chat_destinations WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The address to post to, for the sender and nothing else.
    pub async fn open_chat_url(&self, id: &str) -> Result<Option<String>> {
        let sealed = sqlx::query_scalar::<_, String>("SELECT sealed_url FROM chat_destinations WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.reader())
            .await?;
        Ok(sealed.and_then(|sealed| self.secrets.open(&sealed, &url_context(id)).ok()))
    }

    /// Takes the right to send one digest of a kind in a window. `false` when it was already taken, so it is not sent twice.
    pub async fn claim_chat_digest(&self, id: &str, kind: &str, window: &str) -> Result<bool> {
        let done = sqlx::query(
            "INSERT OR IGNORE INTO chat_digest_log (destination_id, digest_kind, window_key, claimed_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(id)
        .bind(kind)
        .bind(window)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    /// Gives the right back after a failed send, so the next pass tries again.
    pub async fn release_chat_digest(&self, id: &str, kind: &str, window: &str) -> Result<()> {
        sqlx::query("DELETE FROM chat_digest_log WHERE destination_id = ?1 AND digest_kind = ?2 AND window_key = ?3")
            .bind(id)
            .bind(kind)
            .bind(window)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn record_chat_result(&self, id: &str, error: Option<&str>) -> Result<()> {
        sqlx::query(
            "UPDATE chat_destinations SET last_error = ?2, last_sent_at = CASE WHEN ?2 IS NULL THEN ?3 ELSE last_sent_at END WHERE id = ?1",
        )
        .bind(id)
        .bind(error.map(|text| text.chars().take(300).collect::<String>()))
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Items past their review date in a workspace.
    pub async fn count_due_reviews(&self, workspace_id: &str) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM review_assignments WHERE workspace_id = ?1 AND next_review_at IS NOT NULL AND next_review_at <= ?2",
        )
        .bind(workspace_id)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(self.reader())
        .await?)
    }

    /// Tasks that are not done and past their due date in a workspace.
    pub async fn count_overdue_tasks(&self, workspace_id: &str) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM workspace_tasks WHERE workspace_id = ?1 AND status != 'done' AND due_at IS NOT NULL AND due_at < ?2",
        )
        .bind(workspace_id)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(self.reader())
        .await?)
    }
}
