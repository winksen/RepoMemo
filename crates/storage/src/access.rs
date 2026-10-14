//! Ways into an account other than typing a password: invitation links,
//! password reset links issued by a system administrator, and personal access
//! tokens for scripts.
//!
//! Every secret is random, returned once, and stored only as its SHA-256, the
//! same way refresh tokens are. Links are single-use; tokens last until they
//! expire or are revoked.

use anyhow::Result;
use chrono::{Duration, Utc};
use repomemo_domain::{Invitation, PersonalAccessToken, SharedUser};
use uuid::Uuid;

use super::{hash_refresh_token as hash_secret, normalize_email, StorageEngine};

/// Personal access tokens start with this, so they are recognisable in
/// configuration files and secret scanners, and easy to tell from a session.
pub const PERSONAL_TOKEN_PREFIX: &str = "rmp_";
/// Characters of the token shown after the prefix to tell tokens apart.
const VISIBLE_TOKEN_CHARS: usize = 8;
/// A token's last use is written at most this often, so scripts that make
/// many calls do not write to the database on every one.
const LAST_USED_RESOLUTION_MINUTES: i64 = 5;

fn new_secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// An invitation with what happened to it, for accepting or explaining it.
#[derive(Debug, Clone)]
pub struct StoredInvitation {
    pub invitation: Invitation,
    pub accepted_at: Option<String>,
    pub revoked_at: Option<String>,
}

impl StoredInvitation {
    /// Why the invitation cannot be used any more, if it cannot.
    pub fn unusable_reason(&self) -> Option<&'static str> {
        if self.accepted_at.is_some() {
            Some("This invitation has already been used.")
        } else if self.revoked_at.is_some() {
            Some("This invitation was withdrawn. Ask for a new one.")
        } else if self.invitation.expires_at < Utc::now().to_rfc3339() {
            Some("This invitation has expired. Ask for a new one.")
        } else {
            None
        }
    }
}

/// What a valid personal access token grants.
#[derive(Debug, Clone)]
pub struct PersonalTokenGrant {
    pub token_id: String,
    pub user_id: String,
    pub scope: String,
    pub expires_at: String,
}

/// A password reset link, looked up by its secret.
#[derive(Debug, Clone)]
pub struct PasswordReset {
    pub user: SharedUser,
    pub expires_at: String,
    pub used: bool,
}

type InvitationRow = (
    String,         // id
    String,         // email
    String,         // target_type
    String,         // target_id
    String,         // role
    Option<String>, // invited_by id
    Option<String>, // invited_by display name
    Option<String>, // invited_by email
    String,         // created_at
    String,         // expires_at
    Option<String>, // accepted_at
    Option<String>, // revoked_at
);

const INVITATION_COLUMNS: &str = "i.id, i.email, i.target_type, i.target_id, i.role, u.id, u.display_name, u.email, i.created_at, i.expires_at, i.accepted_at, i.revoked_at";

fn invitation_from_row(row: InvitationRow) -> StoredInvitation {
    let (
        id,
        email,
        target_type,
        target_id,
        role,
        by_id,
        by_name,
        by_email,
        created_at,
        expires_at,
        accepted_at,
        revoked_at,
    ) = row;
    StoredInvitation {
        invitation: Invitation {
            id,
            email,
            target_type,
            target_id,
            role,
            invited_by: by_id.map(|id| SharedUser {
                id,
                display_name: by_name.unwrap_or_default(),
                email: by_email,
            }),
            created_at,
            expires_at,
        },
        accepted_at,
        revoked_at,
    }
}

type TokenRow = (String, String, String, String, String, String, Option<String>);

fn token_from_row((id, name, prefix, scope, created_at, expires_at, last_used_at): TokenRow) -> PersonalAccessToken {
    PersonalAccessToken {
        id,
        name,
        prefix,
        scope,
        created_at,
        expires_at,
        last_used_at,
    }
}

impl StorageEngine {
    // ---------------------------------------------------------------- invitations

    /// Creates an invitation and returns it with the raw link token. A pending
    /// invitation of the same email to the same target is withdrawn, so only
    /// the newest link works.
    pub async fn create_invitation(
        &self,
        email: &str,
        target_type: &str,
        target_id: &str,
        role: &str,
        invited_by: &str,
        ttl_days: i64,
    ) -> Result<(Invitation, String)> {
        if !matches!(target_type, "organization" | "workspace") {
            repomemo_domain::bail_invalid!("An invitation is for an organization or a workspace.");
        }
        let email = normalize_email(email)?;
        let id = Uuid::new_v4().to_string();
        let raw = new_secret();
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE invitations SET revoked_at = ?1 WHERE email = ?2 AND target_type = ?3 AND target_id = ?4 AND accepted_at IS NULL AND revoked_at IS NULL",
        )
        .bind(now.to_rfc3339())
        .bind(&email)
        .bind(target_type)
        .bind(target_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO invitations (id, token_hash, email, target_type, target_id, role, invited_by_user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&id)
        .bind(hash_secret(&raw))
        .bind(&email)
        .bind(target_type)
        .bind(target_id)
        .bind(role)
        .bind(invited_by)
        .bind(now.to_rfc3339())
        .bind((now + Duration::days(ttl_days)).to_rfc3339())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let invitation = self
            .find_invitation(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invitation could not be loaded."))?
            .invitation;
        Ok((invitation, raw))
    }

    /// Invitations to a target that can still be accepted, newest first.
    pub async fn list_pending_invitations(&self, target_type: &str, target_id: &str) -> Result<Vec<Invitation>> {
        let rows = sqlx::query_as::<_, InvitationRow>(&format!(
            "SELECT {INVITATION_COLUMNS} FROM invitations i LEFT JOIN users u ON u.id = i.invited_by_user_id WHERE i.target_type = ?1 AND i.target_id = ?2 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > ?3 ORDER BY i.created_at DESC"
        ))
        .bind(target_type)
        .bind(target_id)
        .bind(Utc::now().to_rfc3339())
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| invitation_from_row(row).invitation)
            .collect())
    }

    pub async fn find_invitation(&self, id: &str) -> Result<Option<StoredInvitation>> {
        let row = sqlx::query_as::<_, InvitationRow>(&format!(
            "SELECT {INVITATION_COLUMNS} FROM invitations i LEFT JOIN users u ON u.id = i.invited_by_user_id WHERE i.id = ?1"
        ))
        .bind(id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(invitation_from_row))
    }

    pub async fn find_invitation_by_token(&self, raw: &str) -> Result<Option<StoredInvitation>> {
        let row = sqlx::query_as::<_, InvitationRow>(&format!(
            "SELECT {INVITATION_COLUMNS} FROM invitations i LEFT JOIN users u ON u.id = i.invited_by_user_id WHERE i.token_hash = ?1"
        ))
        .bind(hash_secret(raw.trim()))
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(invitation_from_row))
    }

    /// The invited place's name, its organization's name and id, or `None`
    /// when the target no longer exists.
    pub async fn invitation_target(
        &self,
        target_type: &str,
        target_id: &str,
    ) -> Result<Option<(String, String, String)>> {
        let query = match target_type {
            "workspace" => "SELECT w.name, o.name, o.id FROM workspaces w JOIN workspace_organizations wo ON wo.workspace_id = w.id JOIN organizations o ON o.id = wo.organization_id WHERE w.id = ?1",
            "organization" => "SELECT o.name, o.name, o.id FROM organizations o WHERE o.id = ?1",
            _ => return Ok(None),
        };
        Ok(sqlx::query_as::<_, (String, String, String)>(query)
            .bind(target_id)
            .fetch_optional(self.reader())
            .await?)
    }

    /// Withdraws a pending invitation. Returns whether one was withdrawn.
    pub async fn revoke_invitation(&self, id: &str) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE invitations SET revoked_at = ?1 WHERE id = ?2 AND accepted_at IS NULL AND revoked_at IS NULL",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(changed.rows_affected() > 0)
    }

    /// Marks an invitation used, once: returns false when it was used,
    /// withdrawn or expired in the meantime.
    pub async fn mark_invitation_accepted(&self, id: &str, user_id: &str) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query(
            "UPDATE invitations SET accepted_at = ?1, accepted_by_user_id = ?2 WHERE id = ?3 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > ?1",
        )
        .bind(&now)
        .bind(user_id)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(changed.rows_affected() > 0)
    }

    /// Gives a marked invitation back if joining failed afterwards, so the
    /// link can be tried again.
    pub async fn unmark_invitation_accepted(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE invitations SET accepted_at = NULL, accepted_by_user_id = NULL WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------ password resets

    /// Issues a reset link for a user and returns its raw token and expiry.
    /// Earlier unused links of that user stop working.
    pub async fn create_password_reset(
        &self,
        user_id: &str,
        created_by: &str,
        ttl_hours: i64,
    ) -> Result<(String, String)> {
        let raw = new_secret();
        let now = Utc::now();
        let expires_at = (now + Duration::hours(ttl_hours)).to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM password_resets WHERE user_id = ?1 AND used_at IS NULL")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO password_resets (token_hash, user_id, created_by_user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(hash_secret(&raw))
        .bind(user_id)
        .bind(created_by)
        .bind(now.to_rfc3339())
        .bind(&expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((raw, expires_at))
    }

    pub async fn find_password_reset(&self, raw: &str) -> Result<Option<PasswordReset>> {
        let row = sqlx::query_as::<_, (String, String, Option<String>, String, Option<String>)>(
            "SELECT u.id, u.display_name, u.email, r.expires_at, r.used_at FROM password_resets r JOIN users u ON u.id = r.user_id WHERE r.token_hash = ?1",
        )
        .bind(hash_secret(raw.trim()))
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(|(id, display_name, email, expires_at, used_at)| PasswordReset {
            user: SharedUser {
                id,
                display_name,
                email,
            },
            expires_at,
            used: used_at.is_some(),
        }))
    }

    /// Uses a reset link, once. Returns its user, or `None` when the link is
    /// unknown, used or expired.
    pub async fn consume_password_reset(&self, raw: &str) -> Result<Option<String>> {
        let now = Utc::now().to_rfc3339();
        let user_id = sqlx::query_scalar::<_, String>(
            "UPDATE password_resets SET used_at = ?1 WHERE token_hash = ?2 AND used_at IS NULL AND expires_at > ?1 RETURNING user_id",
        )
        .bind(&now)
        .bind(hash_secret(raw.trim()))
        .fetch_optional(&self.pool)
        .await?;
        Ok(user_id)
    }

    /// Deletes invitations and reset links that expired before `cutoff`.
    pub async fn purge_expired_access_links(&self, cutoff: &str) -> Result<u64> {
        let invitations = sqlx::query("DELETE FROM invitations WHERE expires_at < ?1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?
            .rows_affected();
        let resets = sqlx::query("DELETE FROM password_resets WHERE expires_at < ?1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(invitations + resets)
    }

    // ------------------------------------------------------ personal access tokens

    /// Creates a token and returns it with its raw value, shown only once.
    pub async fn create_personal_token(
        &self,
        user_id: &str,
        name: &str,
        scope: &str,
        ttl_days: i64,
    ) -> Result<(PersonalAccessToken, String)> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            repomemo_domain::bail_invalid!("Token name must be between 1 and 80 characters.");
        }
        if !matches!(scope, "read" | "write" | "system") {
            repomemo_domain::bail_invalid!("Token scope must be read, write or system.");
        }
        let id = Uuid::new_v4().to_string();
        let secret = new_secret();
        let raw = format!("{PERSONAL_TOKEN_PREFIX}{secret}");
        let prefix = format!("{PERSONAL_TOKEN_PREFIX}{}", &secret[..VISIBLE_TOKEN_CHARS]);
        let now = Utc::now();
        let token = PersonalAccessToken {
            id: id.clone(),
            name: name.to_owned(),
            prefix: prefix.clone(),
            scope: scope.to_owned(),
            created_at: now.to_rfc3339(),
            expires_at: (now + Duration::days(ttl_days)).to_rfc3339(),
            last_used_at: None,
        };
        sqlx::query(
            "INSERT INTO personal_access_tokens (id, user_id, name, token_hash, prefix, scope, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(&token.name)
        .bind(hash_secret(&raw))
        .bind(&prefix)
        .bind(scope)
        .bind(&token.created_at)
        .bind(&token.expires_at)
        .execute(&self.pool)
        .await?;
        Ok((token, raw))
    }

    /// The user's tokens that are not revoked, expired ones included (the
    /// owner sees that they expired), newest first.
    pub async fn list_personal_tokens(&self, user_id: &str) -> Result<Vec<PersonalAccessToken>> {
        let rows = sqlx::query_as::<_, TokenRow>(
            "SELECT id, name, prefix, scope, created_at, expires_at, last_used_at FROM personal_access_tokens WHERE user_id = ?1 AND revoked_at IS NULL ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(token_from_row).collect())
    }

    /// Revokes one of the user's tokens. Returns whether it existed.
    pub async fn revoke_personal_token(&self, user_id: &str, token_id: &str) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE personal_access_tokens SET revoked_at = ?1 WHERE id = ?2 AND user_id = ?3 AND revoked_at IS NULL",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(token_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(changed.rows_affected() > 0)
    }

    /// Revokes every token of a user. Returns how many were revoked.
    pub async fn revoke_user_personal_tokens(&self, user_id: &str) -> Result<u64> {
        Ok(
            sqlx::query("UPDATE personal_access_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL")
                .bind(Utc::now().to_rfc3339())
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    /// Checks a raw token: what it grants, or `None` when it is unknown,
    /// revoked or expired. Records its use now and then.
    pub async fn authenticate_personal_token(&self, raw: &str) -> Result<Option<PersonalTokenGrant>> {
        if !raw.starts_with(PERSONAL_TOKEN_PREFIX) {
            return Ok(None);
        }
        let now = Utc::now();
        let row = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT id, user_id, scope, expires_at, last_used_at FROM personal_access_tokens WHERE token_hash = ?1 AND revoked_at IS NULL",
        )
        .bind(hash_secret(raw))
        .fetch_optional(self.reader())
        .await?;
        let Some((token_id, user_id, scope, expires_at, last_used_at)) = row else {
            return Ok(None);
        };
        if expires_at <= now.to_rfc3339() {
            return Ok(None);
        }
        let stale = (now - Duration::minutes(LAST_USED_RESOLUTION_MINUTES)).to_rfc3339();
        if last_used_at.map_or(true, |used| used < stale) {
            sqlx::query("UPDATE personal_access_tokens SET last_used_at = ?1 WHERE id = ?2")
                .bind(now.to_rfc3339())
                .bind(&token_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(Some(PersonalTokenGrant {
            token_id,
            user_id,
            scope,
            expires_at,
        }))
    }
}

impl StorageEngine {
    // ------------------------------------------------------ email verification

    /// The account's address counts as verified: it is, unless something marked it otherwise.
    pub async fn email_is_verified(&self, user_id: &str) -> Result<bool> {
        let unverified = sqlx::query_scalar::<_, Option<String>>("SELECT email_unverified_at FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(self.reader())
            .await?
            .flatten();
        Ok(unverified.is_none())
    }

    pub async fn mark_email_unverified(&self, user_id: &str) -> Result<()> {
        sqlx::query("UPDATE users SET email_unverified_at = ?2 WHERE id = ?1 AND email_unverified_at IS NULL")
            .bind(user_id)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_email_verified(&self, user_id: &str) -> Result<()> {
        sqlx::query("UPDATE users SET email_unverified_at = NULL WHERE id = ?1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A link for the account's address. Earlier unused links of the account stop working.
    pub async fn create_email_verification(
        &self,
        user_id: &str,
        email: &str,
        ttl_hours: i64,
    ) -> Result<(String, String)> {
        let raw = new_secret();
        let now = Utc::now();
        let expires_at = (now + Duration::hours(ttl_hours)).to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM email_verifications WHERE user_id = ?1 AND used_at IS NULL")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO email_verifications (token_hash, user_id, email, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(hash_secret(&raw))
        .bind(user_id)
        .bind(email)
        .bind(now.to_rfc3339())
        .bind(&expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((raw, expires_at))
    }

    /// Uses a link, once: the account whose address it was made for is
    /// verified. `None` when the link is unknown, used, expired, or the
    /// account's address is no longer the one it was sent to.
    pub async fn consume_email_verification(&self, raw: &str) -> Result<Option<(String, String)>> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let found = sqlx::query_as::<_, (String, String)>(
            "UPDATE email_verifications SET used_at = ?1 WHERE token_hash = ?2 AND used_at IS NULL AND expires_at > ?1 RETURNING user_id, email",
        )
        .bind(&now)
        .bind(hash_secret(raw.trim()))
        .fetch_optional(&mut *tx)
        .await?;
        let Some((user_id, email)) = found else {
            return Ok(None);
        };
        let current = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = ?1")
            .bind(&user_id)
            .fetch_optional(&mut *tx)
            .await?;
        if current.as_deref() != Some(email.as_str()) {
            tx.commit().await?;
            return Ok(None);
        }
        sqlx::query("UPDATE users SET email_unverified_at = NULL WHERE id = ?1")
            .bind(&user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some((user_id, email)))
    }

    /// Gives the account another address, unverified. Pending links stop working.
    pub async fn change_user_email(&self, user_id: &str, email: &str) -> Result<SharedUser> {
        let email = normalize_email(email)?;
        let taken = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE email = ?1 AND id != ?2")
            .bind(&email)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        if taken.is_some() {
            repomemo_domain::bail_conflict!("An account already uses that email address.");
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE users SET email = ?2, email_unverified_at = ?3, updated_at = ?3 WHERE id = ?1")
            .bind(user_id)
            .bind(&email)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM email_verifications WHERE user_id = ?1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        let display_name = sqlx::query_scalar::<_, String>("SELECT display_name FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(SharedUser {
            id: user_id.to_owned(),
            email: Some(email),
            display_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn storage() -> (StorageEngine, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("repomemo-access-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(crate::StorageConfig::new(dir.clone()))
            .await
            .unwrap();
        (storage, dir)
    }

    #[tokio::test]
    async fn invitations_are_single_use_and_the_newest_link_wins() {
        let (storage, dir) = storage().await;
        let inviter = storage.create_user("owner@example.com", "Owner", "hash").await.unwrap();
        let (first, first_raw) = storage
            .create_invitation("New@Example.com", "workspace", "ws", "member", &inviter.id, 7)
            .await
            .unwrap();
        assert_eq!(first.email, "new@example.com");
        let (second, second_raw) = storage
            .create_invitation("new@example.com", "workspace", "ws", "viewer", &inviter.id, 7)
            .await
            .unwrap();
        let old = storage.find_invitation_by_token(&first_raw).await.unwrap().unwrap();
        assert!(old.unusable_reason().is_some(), "the older link is withdrawn");
        assert_eq!(
            storage.list_pending_invitations("workspace", "ws").await.unwrap().len(),
            1
        );

        let current = storage.find_invitation_by_token(&second_raw).await.unwrap().unwrap();
        assert_eq!(current.invitation.id, second.id);
        assert!(current.unusable_reason().is_none());
        assert!(storage.mark_invitation_accepted(&second.id, &inviter.id).await.unwrap());
        assert!(
            !storage.mark_invitation_accepted(&second.id, &inviter.id).await.unwrap(),
            "used once only"
        );
        assert!(storage
            .list_pending_invitations("workspace", "ws")
            .await
            .unwrap()
            .is_empty());
        assert!(!storage.revoke_invitation(&first.id).await.unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn reset_links_work_once_and_replace_each_other() {
        let (storage, dir) = storage().await;
        let user = storage
            .create_user("forgot@example.com", "Forgot", "hash")
            .await
            .unwrap();
        let (first, _) = storage.create_password_reset(&user.id, &user.id, 24).await.unwrap();
        let (second, _) = storage.create_password_reset(&user.id, &user.id, 24).await.unwrap();
        assert!(
            storage.find_password_reset(&first).await.unwrap().is_none(),
            "a new link replaces the old one"
        );
        assert_eq!(
            storage.find_password_reset(&second).await.unwrap().unwrap().user.id,
            user.id
        );
        assert_eq!(
            storage.consume_password_reset(&second).await.unwrap().as_deref(),
            Some(user.id.as_str())
        );
        assert_eq!(storage.consume_password_reset(&second).await.unwrap(), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn personal_tokens_authenticate_until_revoked() {
        let (storage, dir) = storage().await;
        let user = storage
            .create_user("script@example.com", "Script", "hash")
            .await
            .unwrap();
        let (token, raw) = storage.create_personal_token(&user.id, "CI", "read", 30).await.unwrap();
        assert!(raw.starts_with(PERSONAL_TOKEN_PREFIX) && raw.starts_with(&token.prefix));
        let grant = storage.authenticate_personal_token(&raw).await.unwrap().unwrap();
        assert_eq!(
            (grant.user_id.as_str(), grant.scope.as_str()),
            (user.id.as_str(), "read")
        );
        assert!(storage.list_personal_tokens(&user.id).await.unwrap()[0]
            .last_used_at
            .is_some());
        assert!(storage
            .authenticate_personal_token("rmp_unknown")
            .await
            .unwrap()
            .is_none());
        assert!(storage.revoke_personal_token(&user.id, &token.id).await.unwrap());
        assert!(storage.authenticate_personal_token(&raw).await.unwrap().is_none());
        assert!(storage.create_personal_token(&user.id, " ", "read", 30).await.is_err());
        assert!(storage.create_personal_token(&user.id, "x", "admin", 30).await.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn an_address_is_verified_once_by_a_link_made_for_it() {
        let (storage, dir) = storage().await;
        let user = storage
            .create_user("verify@example.com", "Verify", "hash")
            .await
            .unwrap();
        assert!(
            storage.email_is_verified(&user.id).await.unwrap(),
            "accounts are verified unless marked"
        );
        storage.mark_email_unverified(&user.id).await.unwrap();
        assert!(!storage.email_is_verified(&user.id).await.unwrap());

        let (first, _) = storage
            .create_email_verification(&user.id, "verify@example.com", 24)
            .await
            .unwrap();
        let (second, _) = storage
            .create_email_verification(&user.id, "verify@example.com", 24)
            .await
            .unwrap();
        assert!(
            storage.consume_email_verification(&first).await.unwrap().is_none(),
            "a newer link replaces it"
        );
        assert_eq!(
            storage.consume_email_verification(&second).await.unwrap().unwrap().1,
            "verify@example.com"
        );
        assert!(storage.email_is_verified(&user.id).await.unwrap());
        assert!(
            storage.consume_email_verification(&second).await.unwrap().is_none(),
            "once"
        );

        // A link made for the old address does nothing after the address changed.
        let (old, _) = storage
            .create_email_verification(&user.id, "verify@example.com", 24)
            .await
            .unwrap();
        storage.change_user_email(&user.id, "other@example.com").await.unwrap();
        assert!(!storage.email_is_verified(&user.id).await.unwrap());
        assert!(storage.consume_email_verification(&old).await.unwrap().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
