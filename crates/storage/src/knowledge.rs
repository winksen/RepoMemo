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

/// A file marked *needs review* whose owner is the person (V0.6.2), with the workspace it is in.
#[derive(Debug, Clone)]
pub struct OwnedReview {
    pub workspace_id: String,
    pub workspace_name: String,
    pub artifact_id: String,
    pub title: String,
    pub reason: String,
    pub since: String,
}

impl StorageEngine {
    /// The files waiting for this person's review across workspaces they can still open: they own the file, its state
    /// is *needs review*, and their membership has not ended. A workspace they left or lost drops out here, so a count
    /// built from this cannot reveal it.
    pub async fn evidence_reviews_owned_by(&self, user_id: &str) -> Result<Vec<OwnedReview>> {
        let rows = sqlx::query(
            r#"
            SELECT lifecycle.workspace_id, workspaces.name AS workspace_name, artifacts.id AS artifact_id,
                   artifacts.title, lifecycle.review_note, lifecycle.updated_at
            FROM artifact_lifecycle AS lifecycle
            JOIN artifacts ON artifacts.id = lifecycle.artifact_id
            JOIN workspaces ON workspaces.id = lifecycle.workspace_id
            JOIN workspace_memberships AS membership
              ON membership.workspace_id = lifecycle.workspace_id AND membership.user_id = ?1
             AND (membership.expires_at IS NULL OR membership.expires_at > ?2)
            WHERE lifecycle.owner_user_id = ?1 AND lifecycle.status = 'needs_review'
            ORDER BY lifecycle.updated_at ASC, artifacts.title ASC
            LIMIT 200
            "#,
        )
        .bind(user_id)
        .bind(Utc::now().to_rfc3339())
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| OwnedReview {
                workspace_id: row.get("workspace_id"),
                workspace_name: row.get("workspace_name"),
                artifact_id: row.get("artifact_id"),
                title: row.get("title"),
                reason: row.get("review_note"),
                since: row.get("updated_at"),
            })
            .collect())
    }

    /// Unread mentions of this person in workspaces they can still open.
    pub async fn unread_mentions_for(&self, user_id: &str) -> Result<Vec<SharedNotification>> {
        let rows = sqlx::query_as::<_, SharedNotificationRow>(
            r#"
            SELECT n.id, n.workspace_id, n.notification_type, n.title, n.body, n.href, n.read_at, n.created_at
            FROM workspace_notifications AS n
            JOIN workspace_memberships AS membership
              ON membership.workspace_id = n.workspace_id AND membership.user_id = ?1
             AND (membership.expires_at IS NULL OR membership.expires_at > ?2)
            WHERE n.user_id = ?1 AND n.notification_type = 'evidence_mention' AND n.read_at IS NULL
            ORDER BY n.created_at DESC, n.id DESC
            LIMIT 100
            "#,
        )
        .bind(user_id)
        .bind(Utc::now().to_rfc3339())
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(SharedNotification::from).collect())
    }
}
