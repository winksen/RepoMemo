//! The life of an account: deactivation and reactivation (roadmap F15a,
//! V0.2.16). A deactivated account keeps everything it wrote and every
//! membership, but cannot sign in or use a token.

use super::*;

/// An organization or workspace an account owns, which stands in the way of
/// deactivating it until the ownership is transferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedPlace {
    /// `organization` or `workspace`.
    pub kind: &'static str,
    pub id: String,
    pub name: String,
}

/// What deactivating an account ended.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deactivation {
    pub personal_tokens: u64,
    pub invitations: u64,
}

impl StorageEngine {
    /// Whether the account is deactivated. An unknown account is not.
    pub async fn is_deactivated(&self, user_id: &str) -> Result<bool> {
        let at = sqlx::query_scalar::<_, Option<String>>("SELECT deactivated_at FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(self.reader())
            .await?
            .flatten();
        Ok(at.is_some())
    }

    /// The organizations and workspaces the account owns, by name.
    pub async fn owned_places(&self, user_id: &str) -> Result<Vec<OwnedPlace>> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            r#"SELECT 'organization', o.id, o.name
               FROM organization_memberships m JOIN organizations o ON o.id = m.organization_id
               WHERE m.user_id = ?1 AND m.role = 'owner'
               UNION ALL
               SELECT 'workspace', w.id, w.name
               FROM workspace_memberships m JOIN workspaces w ON w.id = m.workspace_id
               WHERE m.user_id = ?1 AND m.role = 'owner'
               ORDER BY 3 COLLATE NOCASE ASC, 1 ASC"#,
        )
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(kind, id, name)| OwnedPlace {
                kind: if kind == "organization" { "organization" } else { "workspace" },
                id,
                name,
            })
            .collect())
    }

    /// Deactivates an account in one step: it can no longer sign in, its
    /// sessions, tokens, pending links and the invitations it sent end, and
    /// every access token already issued stops working. Returns `None` when
    /// the account was already deactivated.
    ///
    /// The server always keeps an active system administrator.
    pub async fn deactivate_user(&self, user_id: &str) -> Result<Option<Deactivation>> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE users SET deactivated_at = ?1, session_version = session_version + 1, updated_at = ?1 WHERE id = ?2 AND deactivated_at IS NULL",
        )
        .bind(&now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE id = ?1")
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?;
            if exists == 0 {
                repomemo_domain::bail_not_found!("User was not found.");
            }
            return Ok(None);
        }
        let remaining = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM users WHERE is_system_admin = 1 AND deactivated_at IS NULL",
        )
        .fetch_one(&mut *tx)
        .await?;
        let was_admin = sqlx::query_scalar::<_, i64>("SELECT is_system_admin FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        if was_admin != 0 && remaining == 0 {
            repomemo_domain::bail_conflict!(
                "The last system administrator cannot be deactivated; grant the role to someone else first."
            );
        }
        sqlx::query("UPDATE refresh_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL")
            .bind(&now)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        let personal_tokens = sqlx::query(
            "UPDATE personal_access_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
        )
        .bind(&now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let invitations = sqlx::query(
            "UPDATE invitations SET revoked_at = ?1 WHERE invited_by_user_id = ?2 AND accepted_at IS NULL AND revoked_at IS NULL",
        )
        .bind(&now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        sqlx::query("UPDATE password_resets SET used_at = ?1 WHERE user_id = ?2 AND used_at IS NULL")
            .bind(&now)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE email_verifications SET used_at = ?1 WHERE user_id = ?2 AND used_at IS NULL")
            .bind(&now)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(Deactivation {
            personal_tokens,
            invitations,
        }))
    }

    /// Lets a deactivated account sign in again with its own password.
    /// Returns whether it was deactivated. What deactivating ended (sessions,
    /// tokens, invitations) is not brought back.
    pub async fn reactivate_user(&self, user_id: &str) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE users SET deactivated_at = NULL, updated_at = ?1 WHERE id = ?2 AND deactivated_at IS NOT NULL",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(changed.rows_affected() > 0)
    }
}
