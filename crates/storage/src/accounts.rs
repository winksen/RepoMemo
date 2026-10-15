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

/// What anonymizing an account removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anonymization {
    pub memberships: u64,
    pub notifications: u64,
    pub conversations: u64,
    pub summaries_rewritten: u64,
}

/// The name an anonymized account shows wherever what it wrote is credited.
pub const FORMER_MEMBER: &str = "Former member";

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
            "UPDATE users SET deactivated_at = NULL, updated_at = ?1 WHERE id = ?2 AND deactivated_at IS NOT NULL AND anonymized_at IS NULL",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(changed.rows_affected() > 0)
    }

    /// Whether the account has been anonymized.
    pub async fn is_anonymized(&self, user_id: &str) -> Result<bool> {
        let at = sqlx::query_scalar::<_, Option<String>>("SELECT anonymized_at FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(self.reader())
            .await?
            .flatten();
        Ok(at.is_some())
    }

    /// Removes the personal data of a deactivated account, for good: its
    /// address, name and password, picture, memberships, notices, preferences,
    /// assistant conversations and the mail addressed to it. The account row
    /// stays as "Former member", so what it wrote keeps its creator.
    ///
    /// Its address and name are also taken out of the activity summaries and
    /// audit details that name them. Log files and earlier backups are not
    /// rewritten.
    pub async fn anonymize_user(&self, user_id: &str) -> Result<Anonymization> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let (email, display_name, deactivated_at, anonymized_at) =
            sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
                "SELECT email, display_name, deactivated_at, anonymized_at FROM users WHERE id = ?1",
            )
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("User was not found."))?;
        if anonymized_at.is_some() {
            repomemo_domain::bail_conflict!("This account is already anonymized.");
        }
        if deactivated_at.is_none() {
            repomemo_domain::bail_conflict!("Deactivate the account before anonymizing it.");
        }
        let owned = sqlx::query_scalar::<_, i64>(
            "SELECT (SELECT COUNT(*) FROM organization_memberships WHERE user_id = ?1 AND role = 'owner') + (SELECT COUNT(*) FROM workspace_memberships WHERE user_id = ?1 AND role = 'owner')",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
        if owned > 0 {
            repomemo_domain::bail_conflict!(
                "This account still owns an organization or a workspace. Transfer the ownership first."
            );
        }

        let mut report = Anonymization::default();
        // Text that names them, while the old values are still known.
        report.summaries_rewritten += sqlx::query(
            "UPDATE workspace_activity SET summary = replace(summary, ?1, 'a former member') WHERE instr(lower(summary), lower(?1)) > 0",
        )
        .bind(&email)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        report.summaries_rewritten += sqlx::query(
            "UPDATE system_audit_events SET detail = replace(detail, ?1, 'a former member') WHERE instr(lower(detail), lower(?1)) > 0",
        )
        .bind(&email)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if display_name.chars().count() >= 3 && display_name != FORMER_MEMBER {
            report.summaries_rewritten += sqlx::query(
                "UPDATE workspace_activity SET summary = replace(summary, ?1, ?3) WHERE (actor_user_id = ?2 OR (subject_type = 'user' AND subject_id = ?2)) AND instr(summary, ?1) > 0",
            )
            .bind(&display_name)
            .bind(user_id)
            .bind(FORMER_MEMBER)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            report.summaries_rewritten += sqlx::query(
                "UPDATE system_audit_events SET detail = replace(detail, ?1, ?3) WHERE actor_user_id = ?2 AND instr(detail, ?1) > 0",
            )
            .bind(&display_name)
            .bind(user_id)
            .bind(FORMER_MEMBER)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        }

        // Mail waiting for the address goes; mail already handled loses it.
        sqlx::query("DELETE FROM mail_outbox WHERE lower(to_address) = lower(?1) AND status IN ('queued', 'sending')")
            .bind(&email)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE mail_outbox SET to_address = '(removed)', subject = '(removed)' WHERE lower(to_address) = lower(?1)",
        )
        .bind(&email)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM invitations WHERE lower(email) = lower(?1)")
            .bind(&email)
            .execute(&mut *tx)
            .await?;

        // Their own data.
        report.conversations = sqlx::query("DELETE FROM assistant_conversations WHERE user_id = ?1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        report.notifications = sqlx::query("DELETE FROM workspace_notifications WHERE user_id = ?1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        report.memberships = sqlx::query("DELETE FROM workspace_memberships WHERE user_id = ?1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            + sqlx::query("DELETE FROM organization_memberships WHERE user_id = ?1")
                .bind(user_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        for statement in [
            "DELETE FROM user_avatars WHERE user_id = ?1",
            "DELETE FROM notification_preferences WHERE user_id = ?1",
            "DELETE FROM mail_digests WHERE user_id = ?1",
            "DELETE FROM email_verifications WHERE user_id = ?1",
            "DELETE FROM password_resets WHERE user_id = ?1",
            "DELETE FROM personal_access_tokens WHERE user_id = ?1",
            "DELETE FROM refresh_tokens WHERE user_id = ?1",
            "UPDATE workspace_tasks SET assignee_user_id = NULL WHERE assignee_user_id = ?1",
        ] {
            sqlx::query(statement).bind(user_id).execute(&mut *tx).await?;
        }

        // The row itself: an address nobody can receive mail at, no name, and
        // a password hash that matches nothing.
        let placeholder = format!("former-member-{}@anonymized.invalid", user_id.replace('-', ""));
        sqlx::query(
            "UPDATE users SET email = ?1, display_name = ?2, password_hash = '!', is_system_admin = 0, is_app_admin = 0, email_unverified_at = NULL, anonymized_at = ?3, updated_at = ?3 WHERE id = ?4",
        )
        .bind(placeholder)
        .bind(FORMER_MEMBER)
        .bind(&now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(report)
    }
}
