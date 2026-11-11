//! Daily-knowledge state of the 0.6 line: the checklist's facts and a person's dismissal of it (V0.6.1).

use super::*;

/// What exists in a workspace, counted for the first-value checklist.
#[derive(Debug, Clone, Default)]
pub struct OnboardingFacts {
    pub artifacts: i64,
    /// Files that are searchable: indexed.
    pub indexed: i64,
    /// Memory cards citing at least one passage or file.
    pub cited_cards: i64,
    pub members: i64,
}

impl StorageEngine {
    pub async fn onboarding_facts(&self, workspace_id: &str) -> Result<OnboardingFacts> {
        let (artifacts, indexed) = sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN indexed_at IS NOT NULL THEN 1 ELSE 0 END), 0) FROM artifacts WHERE workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?;
        let cited_cards = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(DISTINCT links.from_id) FROM links
               JOIN memory_cards ON memory_cards.id = links.from_id
               WHERE links.workspace_id = ?1 AND links.from_type = 'memory_card'"#,
        )
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?;
        let members =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workspace_memberships WHERE workspace_id = ?1")
                .bind(workspace_id)
                .fetch_one(self.reader())
                .await?;
        Ok(OnboardingFacts {
            artifacts,
            indexed,
            cited_cards,
            members,
        })
    }

    pub async fn onboarding_dismissed(&self, user_id: &str, workspace_id: &str) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM onboarding_dismissals WHERE user_id = ?1 AND workspace_id = ?2",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?
            > 0)
    }

    /// Hides the checklist from this person (`true`) or shows it again (`false`).
    pub async fn set_onboarding_dismissed(&self, user_id: &str, workspace_id: &str, dismissed: bool) -> Result<()> {
        if dismissed {
            sqlx::query(
                "INSERT OR REPLACE INTO onboarding_dismissals (user_id, workspace_id, dismissed_at) VALUES (?1, ?2, ?3)",
            )
            .bind(user_id)
            .bind(workspace_id)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query("DELETE FROM onboarding_dismissals WHERE user_id = ?1 AND workspace_id = ?2")
                .bind(user_id)
                .bind(workspace_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}
