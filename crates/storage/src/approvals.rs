//! Approval of one revision of a memory card (V0.6.8).

use super::*;

#[derive(Debug, Clone)]
pub struct StoredApproval {
    pub revision: i64,
    /// `approved` or `withdrawn`.
    pub decision: String,
    pub user: Option<SharedUser>,
    pub note: String,
    pub created_at: String,
}

impl StorageEngine {
    /// Every decision about the card, newest first.
    pub async fn memory_approval_history(&self, card_id: &str) -> Result<Vec<StoredApproval>> {
        let rows = sqlx::query(
            r#"
            SELECT approval.revision, approval.decision, approval.note, approval.created_at,
                   users.id AS user_id, users.display_name AS user_name, users.email AS user_email
            FROM memory_card_approvals AS approval
            LEFT JOIN users ON users.id = approval.user_id
            WHERE approval.card_id = ?1
            ORDER BY approval.id DESC
            "#,
        )
        .bind(card_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| {
                let user_id: Option<String> = row.get("user_id");
                StoredApproval {
                    revision: row.get("revision"),
                    decision: row.get("decision"),
                    note: row.get("note"),
                    created_at: row.get("created_at"),
                    user: user_id.map(|id| SharedUser {
                        id,
                        display_name: row.get("user_name"),
                        email: row.get("user_email"),
                    }),
                }
            })
            .collect())
    }

    /// Records a decision about `revision`. An approval is refused with a conflict unless `revision` is still the card's
    /// newest, so a reviewer cannot approve text that has changed since they read it.
    pub async fn record_memory_approval(
        &self,
        card_id: &str,
        revision: i64,
        decision: &str,
        user_id: &str,
        note: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let workspace_id = sqlx::query_scalar::<_, String>("SELECT workspace_id FROM memory_cards WHERE id = ?1")
            .bind(card_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("Memory card was not found."))?;
        let newest = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(revision), 0) FROM memory_card_revisions WHERE card_id = ?1",
        )
        .bind(card_id)
        .fetch_one(&mut *tx)
        .await?;
        if revision != newest {
            repomemo_domain::bail_conflict!(
                "This card is now at revision {newest}; revision {revision} is not the one to decide on. Read the newer text first."
            );
        }
        sqlx::query(
            r#"INSERT INTO memory_card_approvals (card_id, workspace_id, revision, decision, user_id, note, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        )
        .bind(card_id)
        .bind(workspace_id)
        .bind(revision)
        .bind(decision)
        .bind(user_id)
        .bind(note.trim())
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
